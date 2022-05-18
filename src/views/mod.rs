use eventstore::ClientSettings;
use std::io;
use tokio::runtime::Runtime;
use tui::style::{Modifier, Style};

pub mod dashboard;

pub struct Context {
    runtime: Runtime,
    selected_style: Style,
    normal_style: Style,
    client: eventstore::Client,
    op_client: eventstore::operations::Client,
}

impl Context {
    pub fn new(setts: ClientSettings) -> io::Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;

        let (client, op_client) = runtime
            .block_on(async move {
                let client = eventstore::Client::new(setts)?;
                let op_client = eventstore::operations::Client::from(client.clone());

                Ok::<_, eventstore::Error>((client, op_client))
            })
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        Ok(Self {
            runtime,
            client,
            op_client,
            selected_style: Style::default().add_modifier(Modifier::REVERSED),
            normal_style: Style::default().add_modifier(Modifier::REVERSED),
        })
    }

    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }
}
