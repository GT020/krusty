#!/usr/bin/env python3
"""Controlled Kubernetes chaos for exercising krusty's live views.

The script only touches namespaces it creates and labels with
`app.kubernetes.io/managed-by=krusty-chaos`. It seeds Deployments, ConfigMaps,
Secrets, Services and Jobs, then continuously scales, updates, creates and
deletes them at the chosen chaos level.

Examples:
    ./scripts/chaos.py 2
    ./scripts/chaos.py 4 --duration 120 --keep
    ./scripts/chaos.py 5 --namespaces 150 --deployments 10 --replicas 10
    ./scripts/chaos.py --cleanup

Every namespace is named `<prefix>-NNN`. Cleanup only deletes namespaces with
that prefix AND the krusty-chaos managed-by label.
"""

from __future__ import annotations

import argparse
import random
import re
import shutil
import string
import subprocess
import sys
import time
from dataclasses import dataclass, field
from datetime import datetime

MANAGED_LABEL = "app.kubernetes.io/managed-by=krusty-chaos"
PAUSE_IMAGE = "registry.k8s.io/pause:3.10"
JOB_IMAGE = "registry.k8s.io/busybox:1.36"
AUTO_CLEAN_INTERVAL = 60.0

LEVELS = {
    1: dict(namespaces=1, deployments=2, configmaps=4, secrets=3, services=2, jobs=1,
            replicas=2, interval=3.0, actions=1),
    2: dict(namespaces=2, deployments=3, configmaps=6, secrets=4, services=3, jobs=1,
            replicas=3, interval=2.0, actions=2),
    3: dict(namespaces=5, deployments=5, configmaps=10, secrets=6, services=4, jobs=2,
            replicas=4, interval=1.0, actions=4),
    4: dict(namespaces=10, deployments=8, configmaps=16, secrets=10, services=6, jobs=3,
            replicas=6, interval=0.5, actions=8),
    5: dict(namespaces=20, deployments=12, configmaps=24, secrets=15, services=8, jobs=4,
            replicas=8, interval=0.2, actions=16),
}


def now() -> str:
    return datetime.now().strftime("%H:%M:%S")


def log(message: str) -> None:
    print(f"[{now()}] {message}", flush=True)


def token(length: int = 8) -> str:
    return "".join(random.choices(string.ascii_lowercase + string.digits, k=length))


@dataclass
class NamespaceState:
    name: str
    deployments: list[str] = field(default_factory=list)
    configmaps: list[str] = field(default_factory=list)
    secrets: list[str] = field(default_factory=list)
    services: list[str] = field(default_factory=list)
    jobs: list[str] = field(default_factory=list)


class Chaos:
    def __init__(self, args: argparse.Namespace, levels: dict, session: str) -> None:
        self.args = args
        self.levels = levels
        self.session = session
        self.prefix = args.namespace_prefix
        self.context = args.context
        self.max_replicas = args.replicas
        self.namespaces: list[NamespaceState] = []
        self.running = True
        self.last_auto_clean = time.monotonic()
        self.kubectl = shutil.which("kubectl")
        if not self.kubectl:
            raise SystemExit("kubectl not found on PATH")

    # ------------------------------------------------------------------ kubectl

    def cmd(self, *args: str) -> list[str]:
        command = [self.kubectl]
        if self.context:
            command += ["--context", self.context]
        return command + list(args)

    def run(
        self,
        *args: str,
        input: str | None = None,
        check: bool = True,
        quiet: bool = False,
    ) -> subprocess.CompletedProcess[str]:
        result = subprocess.run(
            self.cmd(*args),
            input=input,
            text=True,
            capture_output=True,
        )
        if check and result.returncode != 0:
            detail = result.stderr.strip() or result.stdout.strip()
            raise RuntimeError(f"kubectl {' '.join(args[:3])} failed: {detail}")
        if not quiet and result.stdout.strip():
            print(result.stdout.strip())
        return result

    def apply(self, manifests: list[str]) -> None:
        if not manifests:
            return
        self.run("apply", "-f", "-", input="\n---\n".join(manifests), quiet=True)

    def delete(self, kind: str, namespace: str, name: str) -> None:
        self.run(
            "delete", kind, name, "-n", namespace,
            "--ignore-not-found", "--wait=false", quiet=True,
        )

    # ------------------------------------------------------------------ manifests

    def deployment_yaml(self, namespace: str, name: str, replicas: int) -> str:
        return f"""apiVersion: apps/v1
kind: Deployment
metadata:
  name: {name}
  namespace: {namespace}
  labels:
    app: {name}
    krusty-chaos: "true"
  annotations:
    krusty-chaos/revision: "{token()}"
spec:
  replicas: {replicas}
  selector:
    matchLabels:
      app: {name}
  template:
    metadata:
      labels:
        app: {name}
        krusty-chaos: "true"
      annotations:
        krusty-chaos/revision: "{token()}"
    spec:
      terminationGracePeriodSeconds: 0
      containers:
        - name: pause
          image: {PAUSE_IMAGE}
          resources:
            requests:
              cpu: 1m
              memory: 8Mi
"""

    def configmap_yaml(self, namespace: str, name: str) -> str:
        keys = "\n".join(
            f"  key-{index}.txt: {token(24)}" for index in range(random.randint(2, 8))
        )
        return f"""apiVersion: v1
kind: ConfigMap
metadata:
  name: {name}
  namespace: {namespace}
  labels:
    krusty-chaos: "true"
  annotations:
    krusty-chaos/revision: "{token()}"
data:
{keys}
"""

    def secret_yaml(self, namespace: str, name: str) -> str:
        keys = "\n".join(
            f"  secret-{index}: {token(32)}" for index in range(random.randint(1, 4))
        )
        return f"""apiVersion: v1
kind: Secret
metadata:
  name: {name}
  namespace: {namespace}
  labels:
    krusty-chaos: "true"
  annotations:
    krusty-chaos/revision: "{token()}"
type: Opaque
stringData:
{keys}
"""

    def service_yaml(self, namespace: str, name: str, target: str) -> str:
        return f"""apiVersion: v1
kind: Service
metadata:
  name: {name}
  namespace: {namespace}
  labels:
    krusty-chaos: "true"
  annotations:
    krusty-chaos/revision: "{token()}"
spec:
  type: ClusterIP
  selector:
    app: {target}
  ports:
    - name: http
      port: {random.choice([80, 443, 8080, 9090])}
      targetPort: 8080
"""

    def job_yaml(self, namespace: str, name: str) -> str:
        return f"""apiVersion: batch/v1
kind: Job
metadata:
  name: {name}
  namespace: {namespace}
  labels:
    krusty-chaos: "true"
  annotations:
    krusty-chaos/revision: "{token()}"
spec:
  backoffLimit: 0
  ttlSecondsAfterFinished: 120
  template:
    metadata:
      labels:
        krusty-chaos: "true"
    spec:
      restartPolicy: Never
      containers:
        - name: job
          image: {JOB_IMAGE}
          command: ["sh", "-c", "sleep {random.randint(10, 45)}; echo done"]
          resources:
            requests:
              cpu: 5m
              memory: 16Mi
"""

    # ------------------------------------------------------------------ autoclean

    def planned_namespaces(self) -> set[str]:
        return {
            f"{self.prefix}-{index:03d}"
            for index in range(self.args.namespaces)
        }

    def auto_clean_stale(self) -> None:
        """Remove leftover namespaces from earlier runs with this prefix."""
        if not self.args.auto_clean:
            return
        planned = self.planned_namespaces()
        stale = [name for name in self.discover_namespaces() if name not in planned]
        if not stale:
            return
        log(f"autoclean: removing {len(stale)} stale namespace(s) from earlier runs")
        for name in stale:
            self.run(
                "delete", "namespace", name,
                "--ignore-not-found", "--wait=false", quiet=True,
            )

    def trim(
        self,
        state: NamespaceState,
        names: list[str],
        target: int,
        kind: str,
        kubectl_kind: str,
    ) -> None:
        limit = max(target * 2, target + 4)
        removed = 0
        while len(names) > limit:
            name = names.pop(0)
            self.delete(kubectl_kind, state.name, name)
            removed += 1
        if removed:
            log(f"autoclean: {state.name}: removed {removed} excess {kind}(s)")

    def maybe_auto_clean(self) -> None:
        """Bound resource growth during long runs."""
        if not self.args.auto_clean:
            return
        now = time.monotonic()
        if now - self.last_auto_clean < AUTO_CLEAN_INTERVAL:
            return
        self.last_auto_clean = now
        for state in self.namespaces:
            self.trim(state, state.deployments, self.args.deployments,
                      "deployment", "Deployment")
            self.trim(state, state.configmaps, self.args.configmaps,
                      "configmap", "ConfigMap")
            self.trim(state, state.secrets, self.args.secrets, "secret", "Secret")
            self.trim(state, state.services, self.args.services, "service", "Service")
            self.trim(state, state.jobs, max(self.args.jobs * 3, 6), "job", "Job")

    # ------------------------------------------------------------------ seed

    def ensure_namespaces(self) -> None:
        manifests = []
        for index in range(self.args.namespaces):
            name = f"{self.prefix}-{index:03d}"
            state = NamespaceState(name)
            self.namespaces.append(state)
            manifests.append(f"""apiVersion: v1
kind: Namespace
metadata:
  name: {name}
  labels:
    app.kubernetes.io/managed-by: krusty-chaos
    krusty-chaos/prefix: "{self.prefix}"
    krusty-chaos/session: "{self.session}"
""")
        log(f"creating/updating {len(manifests)} namespaces")
        self.apply(manifests)

    def seed_resources(self) -> None:
        for index, state in enumerate(self.namespaces, start=1):
            manifests: list[str] = []
            for number in range(self.args.deployments):
                name = f"chaos-d-{number:02d}"
                state.deployments.append(name)
                replicas = random.randint(0, self.max_replicas)
                manifests.append(self.deployment_yaml(state.name, name, replicas))
            for number in range(self.args.configmaps):
                name = f"chaos-c-{number:02d}"
                state.configmaps.append(name)
                manifests.append(self.configmap_yaml(state.name, name))
            for number in range(self.args.secrets):
                name = f"chaos-s-{number:02d}"
                state.secrets.append(name)
                manifests.append(self.secret_yaml(state.name, name))
            for number in range(self.args.services):
                target = random.choice(state.deployments)
                name = f"chaos-svc-{number:02d}"
                state.services.append(name)
                manifests.append(self.service_yaml(state.name, name, target))
            if not self.args.skip_jobs:
                for number in range(self.args.jobs):
                    name = f"chaos-j-{number:02d}"
                    state.jobs.append(name)
                    manifests.append(self.job_yaml(state.name, name))
            self.apply(manifests)
            if index % 10 == 0 or index == len(self.namespaces):
                log(f"seeded {index}/{len(self.namespaces)} namespaces")

    # ------------------------------------------------------------------ chaos

    def random_namespace(self) -> NamespaceState:
        return random.choice(self.namespaces)

    def create_resource(self, state: NamespaceState, kind: str | None = None) -> None:
        kind = kind or random.choices(
            ["deployment", "configmap", "secret", "service", "job"],
            weights=[25, 25, 20, 15, 15],
        )[0]
        suffix = token(4)
        if kind == "deployment":
            name = f"chaos-d-{suffix}"
            state.deployments.append(name)
            self.apply([self.deployment_yaml(state.name, name, random.randint(0, self.max_replicas))])
        elif kind == "configmap":
            name = f"chaos-c-{suffix}"
            state.configmaps.append(name)
            self.apply([self.configmap_yaml(state.name, name)])
        elif kind == "secret":
            name = f"chaos-s-{suffix}"
            state.secrets.append(name)
            self.apply([self.secret_yaml(state.name, name)])
        elif kind == "service":
            target = random.choice(state.deployments) if state.deployments else f"chaos-d-{suffix}"
            name = f"chaos-svc-{suffix}"
            state.services.append(name)
            self.apply([self.service_yaml(state.name, name, target)])
        else:
            if self.args.skip_jobs:
                return
            name = f"chaos-j-{suffix}"
            state.jobs.append(name)
            self.apply([self.job_yaml(state.name, name)])
        log(f"{state.name}: create {kind} {name}")

    def delete_resource(self, state: NamespaceState) -> None:
        kinds = [
            ("deployment", state.deployments, "Deployment"),
            ("configmap", state.configmaps, "ConfigMap"),
            ("secret", state.secrets, "Secret"),
            ("service", state.services, "Service"),
            ("job", state.jobs, "Job"),
        ]
        if self.args.skip_jobs:
            kinds = kinds[:-1]
        candidates = [entry for entry in kinds if entry[1]]
        if not candidates:
            return self.create_resource(state)
        kind, names, kubectl_kind = random.choices(
            candidates, weights=[30, 25, 20, 15, 10][: len(candidates)]
        )[0]
        name = random.choice(names)
        names.remove(name)
        self.delete(kubectl_kind, state.name, name)
        log(f"{state.name}: delete {kind} {name}")

    def scale_resource(self, state: NamespaceState) -> None:
        if not state.deployments:
            return self.create_resource(state, "deployment")
        name = random.choice(state.deployments)
        replicas = random.randint(0, self.max_replicas)
        self.run(
            "scale", "deployment", name, "-n", state.name,
            f"--replicas={replicas}", quiet=True,
        )
        log(f"{state.name}: scale deployment {name} -> {replicas}")

    def update_resource(self, state: NamespaceState) -> None:
        choices = []
        if state.deployments:
            choices += [("deployment", 20)]
        if state.configmaps:
            choices += [("configmap", 30)]
        if state.secrets:
            choices += [("secret", 25)]
        if state.services:
            choices += [("service", 25)]
        if not choices:
            return self.create_resource(state)
        kind = random.choices(
            [entry[0] for entry in choices],
            weights=[entry[1] for entry in choices],
        )[0]
        if kind == "deployment":
            name = random.choice(state.deployments)
            # Update pod template metadata so the Deployment emits a rollout.
            self.apply([self.deployment_yaml(state.name, name, random.randint(0, self.max_replicas))])
            log(f"{state.name}: update deployment {name}")
        elif kind == "configmap":
            name = random.choice(state.configmaps)
            self.apply([self.configmap_yaml(state.name, name)])
            log(f"{state.name}: update configmap {name}")
        elif kind == "secret":
            name = random.choice(state.secrets)
            self.apply([self.secret_yaml(state.name, name)])
            log(f"{state.name}: update secret {name}")
        else:
            name = random.choice(state.services)
            target = random.choice(state.deployments) if state.deployments else name
            self.apply([self.service_yaml(state.name, name, target)])
            log(f"{state.name}: update service {name}")

    def reconcile(self, state: NamespaceState) -> None:
        """Refill deleted resource types so chaos keeps running over time."""
        if len(state.deployments) < self.args.deployments:
            name = f"chaos-d-{token(4)}"
            state.deployments.append(name)
            self.apply([self.deployment_yaml(state.name, name, random.randint(0, self.max_replicas))])
        if len(state.configmaps) < max(1, self.args.configmaps // 2):
            name = f"chaos-c-{token(4)}"
            state.configmaps.append(name)
            self.apply([self.configmap_yaml(state.name, name)])
        if len(state.secrets) < max(1, self.args.secrets // 2):
            name = f"chaos-s-{token(4)}"
            state.secrets.append(name)
            self.apply([self.secret_yaml(state.name, name)])
        if len(state.services) < max(1, self.args.services // 2):
            target = random.choice(state.deployments) if state.deployments else f"chaos-d-{token(4)}"
            name = f"chaos-svc-{token(4)}"
            state.services.append(name)
            self.apply([self.service_yaml(state.name, name, target)])

    def step(self) -> None:
        for _ in range(self.args.actions):
            action = random.choices(
                ["scale", "update", "delete", "create"],
                weights=[35, 30, 20, 15],
            )[0]
            state = self.random_namespace()
            if action == "scale":
                self.scale_resource(state)
            elif action == "update":
                self.update_resource(state)
            elif action == "delete":
                self.delete_resource(state)
            else:
                self.create_resource(state)
        for state in random.sample(self.namespaces, k=min(3, len(self.namespaces))):
            self.reconcile(state)
        self.maybe_auto_clean()

    def loop(self) -> None:
        deadline = time.monotonic() + self.args.duration if self.args.duration else None
        log(
            f"chaos running: level={self.args.level} namespaces={len(self.namespaces)} "
            f"interval={self.args.interval}s actions/step={self.args.actions} "
            f"(Ctrl-C to stop)"
        )
        while self.running:
            started = time.monotonic()
            try:
                self.step()
            except Exception as error:  # keep the chaos going through transient API errors
                log(f"step failed (continuing): {error}")
            if deadline is not None and time.monotonic() >= deadline:
                log("duration reached")
                return
            elapsed = time.monotonic() - started
            time.sleep(max(0.0, self.args.interval - elapsed))

    # ------------------------------------------------------------------ cleanup

    def cleanup(self) -> None:
        names = [state.name for state in self.namespaces]
        if not names:
            names = self.discover_namespaces()
        if not names:
            log("no chaos namespaces found")
            return
        log(f"deleting {len(names)} chaos namespaces")
        for index, name in enumerate(names, start=1):
            self.run(
                "delete", "namespace", name,
                "--ignore-not-found", "--wait=false", quiet=True,
            )
            if index % 20 == 0 or index == len(names):
                log(f"delete requested for {index}/{len(names)} namespaces")

    def discover_namespaces(self) -> list[str]:
        result = self.run(
            "get", "namespaces",
            "-l", MANAGED_LABEL,
            "-o", "jsonpath={.items[*].metadata.name}",
            quiet=True,
        )
        names = result.stdout.split()
        return [name for name in names if name.startswith(f"{self.prefix}-")]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Create, update, scale and delete Kubernetes resources to exercise krusty.",
        formatter_class=argparse.ArgumentDefaultsHelpFormatter,
    )
    parser.add_argument("level", nargs="?", type=int, choices=sorted(LEVELS), default=2)
    parser.add_argument("--namespace-prefix", default="krusty-chaos",
                        help="namespace names are <prefix>-000, <prefix>-001, ...")
    parser.add_argument("--namespaces", type=int)
    parser.add_argument("--deployments", type=int)
    parser.add_argument("--configmaps", type=int)
    parser.add_argument("--secrets", type=int)
    parser.add_argument("--services", type=int)
    parser.add_argument("--jobs", type=int)
    parser.add_argument("--replicas", type=int, help="maximum replicas for a deployment")
    parser.add_argument("--interval", type=float, help="seconds between chaos steps")
    parser.add_argument("--actions", type=int, help="actions per chaos step")
    parser.add_argument("--duration", type=float, default=0,
                        help="stop after N seconds (0 runs until Ctrl-C)")
    parser.add_argument("--context", help="kubectl context to use")
    parser.add_argument("--skip-jobs", action="store_true",
                        help="do not create Jobs (avoids pulling a second image)")
    parser.add_argument("--keep", action="store_true",
                        help="leave chaos namespaces after stopping")
    parser.add_argument("--auto-clean", action=argparse.BooleanOptionalAction, default=True,
                        help="remove stale namespaces at startup and bound resource growth")
    parser.add_argument("--cleanup", action="store_true",
                        help="delete chaos namespaces for this prefix and exit")
    parser.add_argument("--yes", action="store_true", help="skip confirmation")
    parser.add_argument("--dry-run", action="store_true", help="print the plan and exit")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    defaults = LEVELS[args.level]
    for key in ("namespaces", "deployments", "configmaps", "secrets", "services", "jobs",
                "replicas", "interval", "actions"):
        if getattr(args, key) is None:
            setattr(args, key, defaults[key])
    if args.skip_jobs:
        args.jobs = 0
    if args.interval < 0 or args.replicas < 0 or args.namespaces < 1:
        print("interval, replicas and namespaces must be non-negative/positive", file=sys.stderr)
        return 2
    if not re.fullmatch(r"[a-z0-9]([-a-z0-9]*[a-z0-9])?", args.namespace_prefix):
        print("--namespace-prefix must be a valid lowercase DNS label", file=sys.stderr)
        return 2

    estimated_pods = args.namespaces * args.deployments * args.replicas
    session = token(6)

    if args.cleanup:
        chaos = Chaos(args, defaults, session)
        if not args.yes:
            if not sys.stdin.isatty():
                print("refusing non-interactive cleanup without --yes", file=sys.stderr)
                return 1
            answer = input(
                f"Delete all '{args.namespace_prefix}-*' chaos namespaces? [y/N] "
            ).strip().lower()
            if answer not in ("y", "yes"):
                return 1
        chaos.cleanup()
        return 0

    if args.dry_run:
        print("krusty chaos plan")
        print(f"  context:            {args.context or '<current kubectl context>'}")
        print(f"  level:              {args.level}")
        print(f"  namespaces:         {args.namespaces}  ({args.namespace_prefix}-000 .. "
              f"{args.namespace_prefix}-{args.namespaces - 1:03d})")
        print(f"  deployments/ns:     {args.deployments}")
        print(f"  configmaps/ns:      {args.configmaps}")
        print(f"  secrets/ns:         {args.secrets}")
        print(f"  services/ns:        {args.services}")
        print(f"  jobs/ns:            {args.jobs}")
        print(f"  max replicas:       {args.replicas}")
        print(f"  estimated pods:     {estimated_pods}")
        print(f"  interval:           {args.interval}s")
        print(f"  actions/step:       {args.actions}")
        print(f"  duration:           {args.duration or 'until Ctrl-C'}")
        print(f"  autoclean:          {args.auto_clean} (stale namespaces + bounded growth)")
        print(f"  cleanup on exit:    {not args.keep}")
        return 0

    chaos = Chaos(args, defaults, session)
    print("krusty chaos")
    print(f"  context:        {args.context or '<current kubectl context>'}")
    print(f"  level:          {args.level}")
    print(f"  namespaces:     {args.namespaces} ({args.namespace_prefix}-*)")
    print(f"  estimated pods: {estimated_pods}")
    print(f"  interval:       {args.interval}s, actions/step={args.actions}")
    print(f"  autoclean:      {'on' if args.auto_clean else 'off'} (stale namespaces + growth limit)")
    print(f"  cleanup:        {'no (--keep)' if args.keep else 'yes on exit'}")
    if estimated_pods > 1000:
        print(f"  WARNING: {estimated_pods} pause pods is a large load; make sure the cluster can take it.")
    print("  Only namespaces labeled app.kubernetes.io/managed-by=krusty-chaos are touched.")

    if not args.yes:
        if not sys.stdin.isatty():
            print("refusing non-interactive run without --yes", file=sys.stderr)
            return 1
        answer = input('Type "chaos" to continue: ').strip()
        if answer != "chaos":
            print("aborted")
            return 1

    try:
        chaos.auto_clean_stale()
        chaos.ensure_namespaces()
        chaos.seed_resources()
        chaos.loop()
    except KeyboardInterrupt:
        print()
        log("stopping chaos")
    except Exception as error:
        log(f"fatal: {error}")
        return 1
    finally:
        if not args.keep:
            chaos.cleanup()
        else:
            names = ", ".join(state.name for state in chaos.namespaces[:5])
            more = "" if len(chaos.namespaces) <= 5 else f" (+{len(chaos.namespaces) - 5} more)"
            log(f"resources kept in: {names}{more}")
            log(f"cleanup later: ./scripts/chaos.py --cleanup --namespace-prefix {args.namespace_prefix}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
