# libfoksalclient

A client library for [foksal](https://codeberg.org/alfazet/foksal). Use it to build your own client(s).

### Usage example

```rust
use libfoksalclient::{client::FoksalClient, error::FoksalError, model::SubscriptionTarget};

let (mut client, mut events) = FoksalClient::connect("localhost", 2137).await?;

// subscribe to receive notifications on changes to foksal's state
client.subscribe(SubscriptionTarget::Queue).await?;
client.subscribe(SubscriptionTarget::Sink).await?;
client.subscribe(SubscriptionTarget::Update).await?;
tokio::spawn(async move {
    while let Some(event) = events.recv().await {
        println!("received event {:?}", event);
    }
});

let state = client.state().await?;
println!("volume: {}", state.volume);

client.toggle().await?;
```
