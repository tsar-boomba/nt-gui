use network_tables::v4::{Client, Config, MessageData, SubscriptionOptions};
use tokio::sync::mpsc;

use std::{net::SocketAddr, thread};

#[derive(Debug)]
pub enum TokioMessage {
    Start(SocketAddr),
    Close,
    Reconnect,
    /// Task receiving sub data ended, should only be sent from inside the sub task
    SubscriptionTerminated,
}

#[derive(Debug)]
pub enum EguiMessage {
    /// Result of sending the Start or Reconnect message
    StartResult(Result<(), network_tables::Error>),
    /// Entry data sent by network tables server
    Message(MessageData),
    /// Client was disconnected
    Disconnect,
    /// Client was reconnected
    Reconnect,
}

/// Starts a thread to run the nt client and send back the subscription results
pub fn start_tokio_thread() -> (mpsc::Sender<TokioMessage>, mpsc::Receiver<EguiMessage>) {
    let (task_sender, task_receiver) = mpsc::channel(16);
    let (egui_sender, data_receiver) = mpsc::channel(256);

    let tokio_task_sender = task_sender.clone();
    thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            // Panic if this fails anyways
            .unwrap();

        // Will panic if future panics, which is the desired behavior
        rt.block_on(client_task(task_receiver, tokio_task_sender, egui_sender));
    });

    (task_sender, data_receiver)
}

async fn client_task(
    mut task_receiver: mpsc::Receiver<TokioMessage>,
    task_sender: mpsc::Sender<TokioMessage>,
    egui_sender: mpsc::Sender<EguiMessage>,
) {
    let mut client: Option<Client> = None;

    // Keep track of if we just closed the client to ignore the subscription terminated from that
    let mut just_closed = false;

    tracing::info!("Started client task");
    while let Some(message) = task_receiver.recv().await {
        match message {
            TokioMessage::Start(new_addr) => {
                egui_sender
                    .send(EguiMessage::StartResult(
                        new_client(new_addr, &task_sender, &egui_sender)
                            .await
                            .map(|new_client| {
                                client = Some(new_client);
                            }),
                    ))
                    .await
                    .unwrap();
            }
            TokioMessage::Close => {
                client = None;
                just_closed = true;
            }
            TokioMessage::Reconnect => {
                if let Some(curr_client) = client.as_ref() {
                    egui_sender
                        .send(EguiMessage::StartResult(
                            new_client(curr_client.server_addr(), &task_sender, &egui_sender)
                                .await
                                .map(|new_client| {
                                    client = Some(new_client);
                                }),
                        ))
                        .await
                        .unwrap();
                }
            }
            TokioMessage::SubscriptionTerminated => {
                if !just_closed {
                    // If just_closed is true, we can ignore this message
                    // Otherwise, we just attempt to reconnect to the server

                    if let Some(curr_client) = client.as_ref() {
                        egui_sender
                            .send(EguiMessage::StartResult(
                                new_client(curr_client.server_addr(), &task_sender, &egui_sender)
                                    .await
                                    .map(|new_client| {
                                        client = Some(new_client);
                                    }),
                            ))
                            .await
                            .unwrap();
                    }
                }
            }
        }
    }
}

async fn new_client(
    addr: SocketAddr,
    task_sender: &mpsc::Sender<TokioMessage>,
    egui_sender: &mpsc::Sender<EguiMessage>,
) -> Result<Client, network_tables::Error> {
    let disconnect_sender = egui_sender.clone();
    let reconnect_sender = egui_sender.clone();

    let client = Client::try_new_w_config(
        addr,
        Config {
            connect_timeout: 500,
            on_disconnect: Box::new(move || {
                disconnect_sender.try_send(EguiMessage::Disconnect).ok();
            }),
            on_reconnect: Box::new(move || {
                reconnect_sender.try_send(EguiMessage::Reconnect).ok();
            }),
            ..Default::default()
        },
    )
    .await?;

    let mut subscription = client
        .subscribe_w_options(
            &[""],
            Some(SubscriptionOptions {
                all: Some(true),
                prefix: Some(true),
                ..Default::default()
            }),
        )
        .await?;
    let message_sender = egui_sender.clone();
    let task_sender = task_sender.clone();

    // Start sub task to receive new data
    tokio::spawn(async move {
        while let Some(message) = subscription.next().await {
            message_sender
                .send(EguiMessage::Message(message))
                .await
                .ok();
        }

        // If this task ends, it is bad, send SubTerminated to notify the main tokio loop
        task_sender
            .send(TokioMessage::SubscriptionTerminated)
            .await
            .ok();
    });

    Ok(client)
}
