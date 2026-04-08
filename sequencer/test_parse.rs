use ethers::types::{Address, U256, Signature, H256};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserTransaction {
    pub from: Address,
    pub to: Address,
    pub value: U256,
    pub nonce: u64,
    pub gas_price: U256,
    pub gas_limit: u64,
    pub signature: Signature,
    pub timestamp: u64,
    #[serde(default)]
    pub boost_bid: Option<U256>,
}

fn main() {
    let json = r#"{
        "from": "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
        "to": "0x0202020202020202020202020202020202020202",
        "value": "0x3e8",
        "nonce": 0,
        "gas_price": "0x3b9aca00",
        "gas_limit": 21000,
        "signature": "0xaabbccddeeff00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff001122334455667788991b",
        "timestamp": 1712573394
    }"#;

    match serde_json::from_str::<UserTransaction>(json) {
        Ok(tx) => println!("Success: {:?}", tx),
        Err(e) => println!("Error: {}", e),
    }
}
