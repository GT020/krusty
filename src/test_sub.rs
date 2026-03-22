use iced::subscription;

pub fn test_channel() -> iced::Subscription<()> {
    struct WatchID;
    iced::subscription::channel(
        std::any::TypeId::of::<WatchID>(),
        10,
        |mut sender| async move {
            use iced::futures::SinkExt; // wait, is SinkExt needed?
            // sender.send(()).await;
        }
    )
}
