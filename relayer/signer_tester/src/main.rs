use clap::Parser;

#[derive(Parser)]
struct Arguments {
    #[clap(short, long, default_value = "2")]
    cid: u32,

    #[clap(short, long, default_value = "1234")]
    port: u32,
}

#[tokio::main]
async fn main() {
    let args = Arguments::parse();
    let mut client = signer_client::Client::new(args.cid, args.port)
        .await
        .expect("Failed to connect to signer");

    let azero_account_id = client
        .azero_account_id()
        .await
        .expect("Failed to get Azero account ID");
    let eth_address = client
        .eth_address()
        .await
        .expect("Failed to get ETH address");

    println!("Azero account ID: {:?}", azero_account_id);
    println!("ETH address: {:?}", eth_address);
}
