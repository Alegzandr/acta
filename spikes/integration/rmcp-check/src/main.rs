use rmcp::{
    ServerHandler, ServiceExt, handler::server::wrapper::Parameters, schemars, tool, tool_handler,
    tool_router, transport::stdio,
};

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct LogParams {
    /// progress | decision | discussion
    r#type: String,
    /// What happened, one to three sentences.
    text: String,
    /// Topic id such as T-42. Defaults to the session focus.
    topic: Option<String>,
}

#[derive(Clone)]
struct Acta;

#[tool_router]
impl Acta {
    #[tool(
        name = "brief",
        description = "Get your bearings: focus topic state and last entries.",
        annotations(title = "Brief", read_only_hint = true)
    )]
    async fn brief(&self) -> String {
        "acta: focus T-42".into()
    }

    #[tool(name = "log", description = "Append a timeline entry to a topic.")]
    async fn log(&self, Parameters(p): Parameters<LogParams>) -> String {
        format!("ok {}#15 {} {}", p.topic.unwrap_or("T-42".into()), p.r#type, p.text.len())
    }
}

#[tool_handler(name = "acta", version = "0.0.0", instructions = "Acta: project tracker and scoped memory.")]
impl ServerHandler for Acta {}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = Acta.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
