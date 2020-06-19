use async_std::io;
use async_std::task;
use async_std::prelude::*;

#[async_std::main]
async fn main() {
    println!("Hello, world!");

    let _peripheral_name = "BT05";
    let _characteristic_uuid = "FF:E1";

    let read_line_future = async {
        let stdin = io::stdin();
        let mut line = String::new();
        match stdin.read_line(&mut line).await {
            Ok(_) => Ok(line),
            Err(e) => Err(e)
        }
    };
    
    let delay_future = task::sleep(std::time::Duration::from_secs(5));

    println!("{:?}", read_line_future.join(delay_future).await);

    // TODO: https://github.com/deviceplug/btleplug

    // TODO: https://github.com/deviceplug/btleplug/blob/master/src/api/mod.rs

    // TODO: https://book.async.rs/
    // TODO: https://docs.rs/async-std/1.6.1/async_std/
}
