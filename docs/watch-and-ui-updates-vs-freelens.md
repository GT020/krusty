# Live updates in krusty vs. Freelens

How each app gets from "Kubernetes object changed" to "the right pixels changed",
and where the two designs differ.

Reference checkouts: `krusty` and `../freelens` (Freelens is a Lens fork).

---

## TL;DR

| Concern | Freelens | krusty |
| --- | --- | --- |
| Transport | `fetch` streaming in the Electron renderer (`KubeApi.watch`) | `kube-rs` watcher on a dedicated tokio runtime |
| Initial load | Full list, one request per selected namespace, run in parallel | Kubernetes 1.27+ streaming list (`sendInitialEvents`) when supported; chunked list (`limit=500` + `continue`) as the fallback |
| Watch request | `?watch=1&resourceVersion=…&timeoutSeconds=600` per namespace | `Api::watch` with `timeout=280s`, bookmark events enabled |
| Reconnect | 1 s after normal stream end, 5 s after an error; `410` reloads then re-watches | 250 ms after stream end; soft backoff 0.5 s → 30 s; `410` relists |
| Batching | Observable event buffer flushed by a mobx `reaction` after **1 s** (events: 5 s) | Timer every **50 ms**, or immediately after **256** pending events |
| State container | mobx `observable.array` mutated/replaced in place | `BTreeMap` + UID index owned by the watch task, never by the UI |
| Hand-off to UI | The UI reads the same observable store directly | Immutable `Arc<ResourceSnapshot>` sent over `tokio::sync::watch` |
| UI invalidation | mobx dependency tracking + `@observer`, React re-render | Entity update + `cx.notify()`, whole list entity re-rendered |
| List virtualization | `VirtualList` renders viewport rows | `gpui-component` `TableState` (`UniformListScrollHandle`) |
| Detail pane freshness | Reads the live store, so it reacts to watch updates | Generic, pod, and node details follow single-object watches; pod/node metrics are merged in every 10 s |
| Watch denied by RBAC | No poll fallback; watch retries every 5 s | Falls back to per-kind polling; preference persisted |
| Watch sharing | Ref-counted shared watch per store for selected namespaces | One task per `(context epoch, kind, scope)` |

The headline difference: **Freelens optimizes for fine-grained reactive UI over a
1-second event batching window; krusty optimizes for a simple, immutable
snapshot boundary flushed on a 50 ms cadence (with immediate escalation on
bursts), plus explicit polling fallbacks.**

---

## Freelens pipeline

```
React view mounts
  └─ KubeWatchApi.subscribeStores()
       ├─ store.loadAll()          → list per namespace (Promise.allSettled)
       │                             → mergeItems() → observable items
       └─ store.subscribe()
            └─ KubeApi.watch(ns)   → streaming newline-delimited JSON
                 └─ eventsBuffer.push(event)
                        │ mobx reaction, delay = 1000 ms
                        ▼
                 updateFromEventsBuffer()
                        │ ADDED / MODIFIED / DELETED, keyed by UID
                        │ items.replace(...) only if something changed
                        ▼
                 @observer list + VirtualList → DOM
```

### 1. Listing

`KubeObjectStore.loadAll()` (`packages/core/src/common/k8s-api/kube-object.store.ts:276`)
delegates to `loadItems()` (`:202`):

- Cluster-scoped or "all namespaces" load: one `api.list()`.
- Otherwise: one `api.list({ namespace })` per selected namespace, all in
  parallel with `Promise.allSettled` (`:240`).
- `mergeItems()` (`:327`) keeps items from namespaces outside the requested
  set, sorts, and replaces the observable array.
- Aborted lists are treated as unmounts, not failures (`:216`, `:312`).

There is no chunking/`continue` support for normal stores. The events store is
the exception: it passes `limit: 1000`
(`packages/core/src/renderer/components/events/store.ts:32`).

### 2. Watch

`KubeApi.watch()` (`packages/utility-features/kube-api/src/kube-api.ts:813`):

- Opens a streaming `fetch` to `getWatchUrl()` with `watch=1`,
  `resourceVersion` for that namespace, and `timeoutSeconds` (default 600).
- Reads the body with a `ReadableStream` reader and reassembles newline-framed
  JSON events (`:930–969`).
- A separate watchdog fires at `timeout * 1.1` and reconnects if the server did
  not close the stream (`:851–877`).
- Stream end → reconnect after 1 s; error → reconnect after 5 s. `401`/`404`
  stop retrying, `410` triggers `loadAll()` and then restarts the watch
  (`kube-object.store.ts:550–595`).
- Every event updates the API object's per-namespace resource version
  (`kube-api.ts:990–1005`).

Watches are started only after the list has populated a resource version:
`watchNamespace()` returns early without one (`kube-object.store.ts:542`).

### 3. Batching

`bindWatchEventsUpdater(delay = 1000)` installs a mobx `reaction` over the
observable `eventsBuffer` (`kube-object.store.ts:480–490`). Events are
**accumulated, not dropped**, for up to a second; the events store overrides the
delay to 5 s.

`updateFromEventsBuffer()` (`:602`):

- Builds a UID → array-index map once per flush.
- `ADDED`/`MODIFIED`: skips an event whose `resourceVersion` equals the stored
  object's; otherwise constructs a new `KubeObject`.
- `DELETED`: writes a tombstone and removes the UID from the index.
- Sorts only when an inserted item or a changed sort key requires it.
- Replaces `items` only if something actually changed, then trims the event
  path to `bufferSize` (default 50,000).

### 4. UI updates

The list is a mobx `@observer`; `ItemListLayout.items` is a `@computed` that
applies filters over `store.getItems()`
(`packages/core/src/renderer/components/item-object-list/list-layout.tsx:283`).
`ItemListLayoutContent` feeds that array to `Table`/`VirtualList`, which only
renders rows in the viewport
(`packages/core/src/renderer/components/item-object-list/content.tsx:483–570`).

Rows and detail components are themselves observers, so a replaced array causes
the list to re-render while individual row/detail reads stay dependency-tracked.

The details drawer resolves the selected object through
`apiManager.getStore(path).getByPath(path)` inside an `asyncComputed`
(`packages/core/src/renderer/components/kube-object-details/current-kube-object-in-details.injectable.ts`).
Because `getByPath()` reads the observable `items` synchronously, the drawer
updates as watch events merge into the store.

Metrics are separate polling loops:

- Pods list: 10 s (`packages/core/src/renderer/components/workloads-pods/pods.tsx:32`).
- Pod details list: 120 s.
- Nodes: 30 s.

### 5. Sharing and cleanup

`KubeWatchApi` (`packages/core/src/renderer/kube-watch-api/kube-watch-api.ts`):

- For the currently selected namespaces, one shared watch per store is
  ref-counted across views (`:115–135`).
- Explicit `namespaces` gets its own watch and re-lists on namespace changes
  (`:140–200`).
- Every subscriber gets an `AbortController`; unmount aborts the watch and
  clears retry timers.

---

## krusty pipeline

```
ViewModel.subscribe(kind, scope)
  └─ Repository (global map keyed by epoch + kind + scope)
       └─ WatchTask on the tokio runtime
            ├─ list chunks (500 at a time) ──┐
            └─ watch stream ─── events ──────┤
                                             ▼
                                      ResourceStore
                                   BTreeMap + UID index
                                             │ publish: 50 ms ticker
                                             │ or 256 pending events
                                             ▼
                               Arc<ResourceSnapshot> (immutable)
                                             │ tokio::watch (latest value only)
                                             ▼
                         ViewModel pump: model.apply_snapshot(); cx.notify()
                                             ▼
                              TableState / UniformList (virtualized)
```

### 1. Subscription sharing

`Repository::subscribe()` (`src/repository/mod.rs`) keys a task on
`(context_epoch, ResourceKind, NamespaceScope)`. A second view with the same
key attaches to the same `watch::Sender`; the entry is held weakly so dropping
the last `Subscription` cancels the background task. A context switch bumps
`context_epoch`, so old tasks are orphaned and dropped with their views.

### 2. Listing

`run_stream()` first tries a Kubernetes 1.27+ streaming list
(`WatchParams::initial_events()`), which sends synthetic `ADDED` events followed
by a `BOOKMARK` carrying `k8s.io/initial-events-end: "true"`. It applies those
events progressively, publishes on the normal 50 ms/256-event cadence, and
continues the regular watch from the bookmark's resource version.

Compatibility fallbacks:

- API server version `< 1.27` → chunked list immediately.
- Watch setup rejected, a watch `ERROR`, an early stream end, or a bookmark
  without the initial-events annotation → chunked list.
- No initial-end bookmark within 5 s idle / 60 s total → chunked list.
- A fallback disables streaming for that watch task, so old servers are probed
  at most once per subscription.

The chunked list path (`WatchTask::list_progressively()` in
`src/repository/resources/watcher.rs`) remains:

- `ListParams::limit(500)` with `continue` tokens.
- First chunk calls `store.begin_sync()`; later chunks upsert.
- Publishes a snapshot after **every chunk**, so a huge list renders
  progressively instead of waiting for the whole response.
- `finish_sync()` removes unseen objects and keeps the list's `resourceVersion`
  to start the watch from.

### 3. Watch loop

`WatchTask::run_stream()` (`watcher.rs:256`):

- `WatchParams::default().timeout(280)`; kube enables bookmark events by
  default, and bookmarks advance the stored resource version.
- `Added`/`Modified` map the typed object to a compact `Item` and call
  `store.upsert`; `Deleted` removes by `(namespace, name)`.
- `Error` with code 410 / reason `Expired`/`Gone` schedules a relist.
- Normal stream end resumes from the last resource version after 250 ms.
- Soft failures (network, 429, 5xx, 20 s request timeout) retry forever with
  exponential backoff from 0.5 s to 30 s (`src/repository/retry.rs`).
- Hard failures (400/401/403/404/405/422) get one retry (two attempts total),
  then the kind is
  marked denied in a process-global registry and the task switches to polling.
- Bookmarked resource versions mean a reconnect does not replay the list.

### 4. Batching and snapshots

`ResourceStore` (`src/models/resources/store.rs:119`) owns:

- `BTreeMap<ObjectKey, Arc<Item>>` — sorted by `(namespace, name)` with no sort
  pass on read.
- `HashMap<uid, ObjectKey>` — O(1) replacement/removal even if metadata changes.
- `pending` counter for throttled flushes.

`WatchTask::publish()` (`watcher.rs:477`) builds an immutable
`ResourceSnapshot { Arc<[Arc<Item>]>, phase, mode }` and calls
`watch::Sender::send_replace`. Publishing happens when:

- a 50 ms ticker fires and anything is pending (`watcher.rs:367`), or
- 256 events have accumulated (`watcher.rs:459`), or
- a list chunk completed, or the mode/phase changed.

`FLUSH_INTERVAL = 50ms`, `FLUSH_AT = 256`, `LIST_CHUNK_SIZE = 500`
(`watcher.rs:39–44`).

Because `tokio::sync::watch` stores only the latest value, a slow consumer
coalesces intermediate snapshots — it never blocks the watch task.

### 5. UI updates

A view model (e.g. `src/view_models/resources/pod/mod.rs:254–320`) keeps the
`Subscription`, applies the initial snapshot to a model entity, then spawns a
GPUI task:

```rust
while let Some(snapshot) = pump.next().await {
    model.update(cx, |model, cx| {
        model.apply_snapshot(snapshot);
        if active.load(Ordering::Relaxed) {
            cx.notify();
        }
    });
}
```

- Inactive tabs pause their `Subscription`; the pump stops receiving snapshots.
  On activation the view model applies `subscription.snapshot()` immediately,
  then resumes the pump, which also returns the latest coalesced snapshot.
- Rendering reads the snapshot through the table delegate (`rows_count`,
  `render_td`, `cell_text`). `TableState` uses a `UniformListScrollHandle`, so
  only viewport rows are built.
- A manual refresh bumps a generation on the task's `refresh` channel, which
  forces a relist.

### 6. Details

- Generic detail panes call `subscribe_detail()` (`src/repository/resources/detail.rs`),
  which GETs the object and then follows it with a single-object watch
  (`fieldSelector=metadata.name=<name>`). On `410` it re-GETs; if watch is
  denied it falls back to a 5 s GET poll. The view model cancels this stream
  while its tab is hidden.
- Pod and node details GET the object once and then follow it with the same
  single-object watch used by generic details, while a 10 s timer fetches
  metrics (`src/repository/resources/pods/mod.rs:455–512`,
  `src/repository/resources/nodes/mod.rs`). Object fields and usage therefore
  stay live together.
- The 10 s metrics sampling matches Freelens' pods-list cadence.

### 7. Failure and preference handling

- `WATCH_DENIED` (`watcher.rs:50–74`) remembers per epoch/kind that the API
  server rejected watch, so a new subscription for the same kind starts polling
  immediately instead of retrying a doomed watch.
- `WatchPreference` / `PollRate` are persisted per kind in
  `~/.config/krusty/poll-rates.json` (`src/repository/watch_prefs.rs`).
  `PollRate::Every(5)` is the fallback; `Off` disables the timer but manual
  refresh still relists.
- The UI can switch a kind between live watch and poll at runtime; every
  running task for that kind receives the new preference.

---

## Stage-by-stage comparison

| Stage | Freelens | krusty |
| --- | --- | --- |
| Network execution | Renderer process (Electron) | Tokio worker threads, separate from the GPUI UI thread |
| Initial list requests | One per selected namespace, parallel | One streaming watch on 1.27+; otherwise one streamed list per scope with sequential chunks |
| Initial list progress | Spinner until `loadAll` resolves | Streaming `ADDED` events or the first 500 list objects appear immediately |
| Resource version | Per-namespace map in `KubeApi` | One version per watch task, advanced by events/bookmarks |
| Watch granularity | One watch per namespace (or one global watch) | One watch for the whole scope |
| Watch timeout | 600 s + 1.1× client watchdog | 280 s |
| Reconnect delay | 1 s normal end, 5 s error | 250 ms normal end, exponential backoff on errors |
| Batch window | 1 s (events 5 s) | 50 ms / 256 events |
| Store mutation | In-place observable array; replace only on change | Background `BTreeMap`; publish immutable snapshot |
| Sort | Explicit sort when a sort key changes | `BTreeMap` keeps rows sorted, no sort pass |
| UID handling | Rebuild UID→index map per flush | Persistent UID→key map |
| UI granularity | Row/detail-level mobx observers | Whole list entity invalidated per snapshot |
| Background views | React components unmount or keep observing; watches are aborted on unmount | Snapshot delivery pauses and detail/metrics work stops; the shared watch remains open |
| Virtualization | `VirtualList` | `TableState` + `UniformListScrollHandle` |
| Detail freshness | Live from store | One-shot, except metrics |
| RBAC watch denial | Retries watch every 5 s indefinitely | Falls back to polling automatically |
| Manual poll mode | None for lists | Per-kind, persisted (`Every(n)` / `Off`) |
| Shared watch | Ref-counted per store for selected namespaces | Keyed per `(epoch, kind, scope)` |
| Cluster/namespace switch | Abort + reload + resubscribe | New subscription key; old task cancelled by drop |
| Coalescing vs. loss | Buffers every event for 1 s; may trim at 50k items | Latest snapshot wins; intermediate snapshots may be skipped |

---

## Connection lifecycle on UI switches

"Invalidated" here means the watch stream is aborted and returning to the view
requires a new list + watch.

| UI action | Freelens | krusty |
| --- | --- | --- |
| Navigate between resource pages (sidebar or sibling tabs) | **Yes.** `ClusterFrameLayout` renders only `currentRouteComponent`; the old page unmounts, `KubeObjectListLayout.componentWillUnmount` disposes its subscriptions, and `KubeWatchApi` aborts the watch. Returning re-lists and re-watches. | **No.** Every opened tab keeps a `WorkspacePane` entity in `Workspace.panes`; switching calls `sync_visibility()`, which pauses snapshot delivery and detail/metrics work. The shared watch keeps running. |
| Switch a sub-tab within a page (`TabLayout` with routes) | **Yes.** The route changes, so the previous route component unmounts. | **No.** There is no route-level page swap; each tab owns its view. |
| Switch cluster tab (Hotbar / cluster frame) | **No.** `ClusterFrameHandler` keeps one iframe per cluster, hides it with CSS (`visibility: hidden`), and only changes the visible frame. Watches in hidden cluster frames keep running. | **Yes.** `switch_context()` keeps the old client while the new one builds, then swaps it and bumps `context_epoch`. Visible tabs resubscribe immediately; hidden tabs drop their subscription and reconnect on activation. |
| Close a workspace tab | N/A (there are no app-level page tabs; leaving the page is the row above). | **Yes**, but only when the last subscription for that `(epoch, kind, scope)` is dropped. |
| Close a cluster | Yes; the iframe is removed after disconnect, so its watchers die with it. | N/A (single active cluster). |
| Change the namespace filter | **Yes.** A mobx `reaction` aborts the controller, unsubscribes, then `loadAll()` + `subscribe()` for the new selection. | **Yes for that tab.** `set_scope()` drops the tab's subscription and creates a new key; a watch for the old key stays alive only if another tab still holds it. |
| Open/close the details pane | No effect on the list watch. The drawer uses the list store. | No effect on the list watch. The detail fetch has its own task; closing cancels it. |
| Window blur / minimize | No pause. Watches keep streaming. | No pause. The visible tab keeps updating; hidden tabs pause snapshot delivery and details. |
| Restore a saved workspace | N/A (routes are not restored as tabs). | Only the **active** tab of each group is instantiated at startup (`ensure_active_panes`); restored hidden tabs connect lazily when first focused. |

Two practical consequences:

1. **Freelens trades freshness for churn on navigation.** Every page visit is a
   fresh list + watch. The shared-watch refcount only helps when multiple
   mounted components use the same store; it does not preserve a watch across
   route changes.
2. **krusty trades watch count for instant tab switches.** Once a tab has been
   visited, it stays subscribed and only pauses snapshot delivery while hidden.
   With many tabs open, this can still mean many concurrent watches. A cluster
   switch drops hidden tabs' subscriptions; they reconnect lazily on activation
   instead of all resubscribing at once.

---

## Optimization opportunities in krusty

Ordered by expected impact per unit of risk. Status is tracked here and updated
as each item lands.

| # | Suggestion | Status | Where it lives |
| --- | --- | --- | --- |
| 1 | Live details / derive from snapshot | **Done** — generic, pod, and node detail panes follow a single-object watch, with metrics merged in for pods/nodes | `repository/resources/{detail,pods,nodes}.rs`, `view_models/resources/detail.rs` |
| 2 | Pause detail/metrics work for inactive tabs | **Done** | `view_models/resources/{pod,node,overview,detail}.rs` |
| 3 | Share per-object and metrics subscriptions | **Partial** — generic detail watchers are ref-counted per `(epoch, kind, namespace, name)`; pod/node detail and metrics remain per-tab | `repository/resources/detail.rs` |
| 4 | Stage cluster switches | **Done** | `cluster/mod.rs` |
| 5 | Explicit hidden-tab policy | **Done** — hidden tabs pause snapshot delivery, cancel detail/metrics, drop their subscription on a cluster switch, and reconnect lazily on activation | `repository/mod.rs`, `view_models/resources/*` |
| 6 | Diff lists instead of `reset` | **Done** | `models/resources/store.rs`, `repository/resources/watcher.rs` |
| 7 | Skip unchanged watch events | **Done** | `models/resources/store.rs` |
| 8 | No-op snapshot / notify suppression | **Done** | `models/resources/store.rs`, `repository/resources/watcher.rs`, `repository/mod.rs` |
| 9 | Bound high-churn kinds (events/leases) | **Done** — Events are capped at 1000 newest rows and Events/Leases default to polling | `models/resources/store.rs`, `repository/{resources/watcher,watch_prefs}.rs` |
| 10 | Adaptive poll backoff | **Done** — unchanged polls double from the configured rate up to 60 s and reset on change or manual refresh | `repository/resources/watcher.rs` |
| 11 | Persistent collection / cheap snapshot | **Dropped** — list diffing means snapshots rebuild only on real changes; no measured need for another collection dependency | — |
| 12 | Row-level invalidation | **Dropped** — GPUI retained-mode element diffing owns row/paint invalidation | — |
| 13 | Streaming lists (`sendInitialEvents`) | **Done** — Kubernetes 1.27+ fast path with version check, idle/total timeouts, and chunked-list fallback; a fallback disables retries for that task | `repository/resources/watcher.rs` |
| 14 | Adaptive flush cadence | **Dropped** — GPUI coalesces paints per frame; the 50 ms data flush is the only throttle needed | — |
| 15 | Measure mixed-scope overlap | **Done** — subscription create/reuse is traced with kind, scope, and active count | `repository/mod.rs` |
| 16 | Re-probe denied watches | **Done** — automatic RBAC denials are retried every 10 minutes; an explicit polling preference is never overridden | `repository/resources/watcher.rs` |

### Boundary rules followed by these changes

- `models/` owns `Item`, `ResourceStore`, diff/pending semantics, and
  snapshot types. It performs no I/O and knows nothing about GPUI.
- `repository/` owns API calls, list/watch loops, retries, and delivery
  semantics such as pausing a subscription. It publishes snapshots only.
- `view_models/` decides when a tab is active, applies snapshots to model
  entities, and starts/cancels detail or metrics streams. It never renders.
- `ui/` only forwards lifecycle events (for example `ResourceListView::set_active`)
  and renders state. No data-store or retry logic was added there.

GPUI retained-mode cleanup: the model no longer carries a `revision` counter
and the view-model pumps no longer compute a `should_notify` flag. A real data
change updates the model and marks the entity dirty; GPUI diffs and paints.
Data-layer batching/pausing remains because it controls network and background
CPU, not painting.

`cargo check` and `cargo test` pass (106 tests), including store diff/trim
tests, adaptive poll backoff, streaming-list version/bookmark gates, and a
paused-subscription delivery test.

### P0 — correctness and obvious waste

1. **Make details live or derive them from the list snapshot. — DONE**
   `repository/resources/detail.rs` GETs the object, then follows it with a
   single-object watch (`fieldSelector=metadata.name=<name>`), relists on `410`,
   and falls back to a 5 s GET poll when watch is denied. Pod and node details
   now do the same while sharing the same select loop with their 10 s metrics
   sampler; one watch keeps object fields current and the metrics timer updates
   usage.

2. **Pause detail and metrics work for inactive tabs. — DONE**
   `set_active(false)` now pauses the list subscription's snapshot delivery and
   cancels the detail stream (Pods, Nodes, and the generic detail pane). Pod and
   node metrics stop with it. The overview cancels its node-metrics poller while
   hidden and restarts it on activation. No snapshot or metrics work is applied
   in the background.

3. **Share per-object and metrics subscriptions. — PARTIAL**
   Generic detail watchers are now ref-counted per
   `(epoch, kind, namespace, name)` in `repository/resources/detail.rs`: one
   watch feeds every subscriber through a broadcast channel, with the latest
   update seeded to late subscribers. Pod/node detail and metrics still create
   one stream per open tab; sharing those needs a separate typed
   detail-plus-metrics subscription keyed the same way.

4. **Stage cluster switches instead of blanking first. — DONE**
   `switch_context()` keeps the old client, data, and epoch until
   `spawn_client_build` succeeds. The successful build swaps `client` and bumps
   `context_epoch` in one update, so views resubscribe once. A failed build
   clears the client and bumps the epoch so the new context shows an error
   instead of stale data from the previous cluster.

5. **Make the hidden-tab policy explicit. — DONE**
   Hidden tabs pause their subscription (`Subscription::set_paused`), cancel
   detail and metrics work, and apply the latest snapshot once when activated.
   On a cluster switch they discard their subscription instead of reconnecting;
   `set_active(true)` notices the missing subscription and re-subscribes against
   the current epoch. Visible tabs resubscribe immediately.

### P1 — poll and relist efficiency

6. **Diff lists instead of `reset`. — DONE**
   `ResourceStore::begin_sync()` / `finish_sync()` merge a paginated list into
   the existing maps: unchanged `(key, uid, resourceVersion)` entries keep their
   `Arc<Item>`, new or changed entries are inserted, and keys absent from the
   completed list are removed once. `list_progressively` uses this path, so a
   poll with no changes produces no pending objects and no snapshot. An
   interrupted list calls `abort_sync()` and never deletes unseen objects.

7. **Skip unchanged watch events. — DONE**
   `ResourceStore::upsert()` returns `false` and leaves `pending` and the UID
   index untouched when the key, UID, and non-empty resource version match the
   stored item. Duplicate `MODIFIED` events and repeated poll entries are
   therefore free.

8. **Do not publish or notify on a no-op snapshot. — DONE**
   The watcher publishes a list chunk or final list only when `pending > 0` (or
   when the first `Ready` phase still has to be emitted). Combined with the
   paused subscription, a stable poll cycle produces no snapshot, no model
   update, and no `cx.notify()`.

9. **Bound high-churn kinds. — DONE**
   `ResourceStore::trim_oldest()` removes the oldest `created_at` rows, and the
   watcher applies a 1000-row cap to Events after each chunk and watch event.
   Events and Leases now default to polling (5 s), matching the existing
   Namespaces default; users can switch any kind to live watch and the choice
   persists. Freelens only limits its initial events list; krusty bounds the
   live list too.

10. **Adaptive poll backoff. — DONE**
    The poll loop counts consecutive unchanged lists and doubles the configured
    interval (`5 s → 10 → 20 → 40 → 60 s`, capped). A changed list, manual
    refresh, or preference change resets to the configured base rate. The UI
    keeps showing the configured rate; the trace logs show the effective delay.

### P2 — large-list rendering and hand-off

11. **Persistent collection / cheap snapshot. — DROPPED**
    After the list diff, a snapshot is rebuilt only when an object actually
    changes, not on every 50 ms tick or every poll. A persistent collection
    would add a dependency and change the snapshot API for no measured win;
    revisit only if profiling shows the O(n) rebuild is hot during a real
    change storm.

12. **Row-level invalidation. — DROPPED**
    GPUI is retained mode and diffs the element tree itself. The view models no
    longer carry a `should_notify`/`revision` gate; a published snapshot updates
    the model and marks the entity dirty, and GPUI decides what to repaint.
    Manual per-row revision tracking would duplicate that machinery.

13. **Use streaming lists where supported. — DONE**
    `run_stream()` checks the API server version once per context epoch and,
    for 1.27+, opens a watch with `initial_events()`. `ADDED`/`MODIFIED` events
    are applied progressively; the `k8s.io/initial-events-end` bookmark ends the
    initial phase and seeds the regular watch's resource version. Any rejection,
    early end, non-initial bookmark, or 5 s idle / 60 s total timeout falls back
    to the chunked list, then disables streaming for that task. Verified against
    the kind cluster (`v1.37`), which returns the expected initial-end bookmark.

14. **Adaptive flush cadence. — DROPPED**
    GPUI already coalesces paints to its frame cadence. The 50 ms repository
    flush limits data hand-off; adding a second render-oriented throttle would
    duplicate the UI framework.

15. **Measure mixed-scope overlap. — DONE**
    `Repository::subscribe_kind` now logs whether a subscription was reused or
    created, including kind, scope, and the active subscription count. That
    makes duplicate `All` vs `One(ns)` watches visible without changing the
    sharing policy.

16. **Re-probe denied watches occasionally. — DONE**
    An automatic watch denial now schedules a 10-minute re-probe from the poll
    loop. It clears the denied marker and returns to the watch path; if the
    watch is still denied it falls back to polling again. If the user explicitly
    chose polling, `prefs.mode == Poll` keeps the re-probe disabled.

### What is already good

- One watch task per `(epoch, kind, scope)`, shared through `Arc`/`Weak`.
- Chunked initial lists with progressive rendering.
- `BTreeMap` ordering (no sort pass) plus a UID index.
- Immutable snapshots with O(1) clone and no lock shared with the UI.
- Soft/hard retry split and a real polling fallback when watch is denied.
- Streaming initial lists on Kubernetes 1.27+ with a chunked-list fallback.
- Hidden tabs pause snapshot delivery and detail/metrics work; the shared watch
  stays open for instant activation.

---

## Trade-offs

### Where Freelens' design wins

- **Fine-grained renders.** Replacing `items` does not force every reader to
  recompute: mobx observers only repaint what they read.
- **Event completeness.** The 1 s buffer applies every event, so short-lived
  objects (create → delete inside one second) are not lost in a snapshot
  collapse.
- **Live details for free.** Because the drawer reads the same store as the
  list, spec/status changes appear without a second request.
- **Per-namespace watches.** A namespace filter change affects only those
  watches; already-watched namespaces keep streaming.

### Where krusty's design wins

- **No UI-thread network or mutation.** All watch and store work stays on tokio;
  the UI only swaps an `Arc`.
- **Cheap hand-off.** Snapshot clone is O(1), and `BTreeMap` removes the sort
  pass.
- **Progressive large lists.** The first 500 objects appear immediately; the
  rest stream in.
- **Predictable degradation.** RBAC-denied watches become polling instead of a
  retry loop; per-kind polling is user-visible and persisted.
- **Bounded latency.** A quiet feed is flushed within 50 ms, and bursts of 256
  events publish immediately rather than waiting for the next tick. Inactive
  tabs cost no rendering.

---

## File map

### krusty

- `src/repository/mod.rs` — subscription registry, sharing, watch preferences.
- `src/repository/resources/watcher.rs` — list/watch loop, batching, polling.
- `src/repository/retry.rs` — soft/hard retry policy, 20 s request bound.
- `src/repository/watch_prefs.rs` — persisted watch/poll preference.
- `src/models/resources/store.rs` — `ResourceStore`, snapshots, modes.
- `src/view_models/resources/pod/mod.rs` — snapshot pump, `cx.notify()` gating.
- `src/repository/resources/detail.rs` — one-shot generic detail fetch.
- `src/repository/resources/pods/mod.rs` — pod detail + 10 s metrics loop.

### Freelens

- `packages/utility-features/kube-api/src/kube-api.ts` — streaming watch,
  retries, resource-version tracking.
- `packages/core/src/common/k8s-api/kube-object.store.ts` — list/merge, event
  buffer, 1 s flush, store updates.
- `packages/core/src/renderer/kube-watch-api/kube-watch-api.ts` — shared and
  per-view subscriptions, namespace reactions.
- `packages/core/src/renderer/components/item-object-list/list-layout.tsx` —
  computed filtering over the store.
- `packages/core/src/renderer/components/item-object-list/content.tsx` —
  virtualized table content.
- `packages/core/src/renderer/components/kube-object-details/current-kube-object-in-details.injectable.ts`
  — live detail lookup from the store.
- `packages/core/src/renderer/components/workloads-pods/pods.tsx` — 10 s pod
  metrics polling.
