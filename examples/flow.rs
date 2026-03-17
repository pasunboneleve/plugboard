use plugboard::domain::NewMessage;
use plugboard::exchange::Exchange;
use plugboard::exchange::sqlite::SqliteExchange;
use plugboard::plugin::command::CommandPlugin;
use plugboard::worker::{WorkerConfig, WorkerHost};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let exchange = SqliteExchange::open(".plugboard/example.db")?;
    exchange.init()?;

    let root = exchange.publish(NewMessage::new("code.generate", "hello world"))?;
    let plugin = CommandPlugin::new(vec!["sh".into(), "-c".into(), "tr a-z A-Z".into()])?;
    let runner = WorkerHost::new(
        &exchange,
        &plugin,
        WorkerConfig::new("code.generate", "code.generated", "code.generate.failed", 5),
    );

    runner.run_once()?;

    for message in exchange.read_by_conversation(&root.conversation_id)? {
        println!("{}: {}", message.topic, message.body);
    }

    Ok(())
}
