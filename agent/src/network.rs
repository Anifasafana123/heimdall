use serde::Serialize;
use sysinfo::Networks;

#[derive(Debug, Serialize)]
pub struct InterfaceMetrics {
    pub name: String,
    pub total_received_bytes: u64,
    pub total_transmitted_bytes: u64,
    pub received_since_last_bytes: u64,
    pub transmitted_since_last_bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct NetworkMetrics {
    pub interfaces: Vec<InterfaceMetrics>,
    pub total_received_bytes: u64,
    pub total_transmitted_bytes: u64,
}

pub fn collect(networks: &mut Networks) -> NetworkMetrics {
    networks.refresh(true);

    let interfaces: Vec<InterfaceMetrics> = networks
        .iter()
        .map(|(name, data)| InterfaceMetrics {
            name: name.clone(),
            total_received_bytes: data.total_received(),
            total_transmitted_bytes: data.total_transmitted(),
            received_since_last_bytes: data.received(),
            transmitted_since_last_bytes: data.transmitted(),
        })
        .collect();

    NetworkMetrics {
        total_received_bytes: interfaces.iter().map(|i| i.total_received_bytes).sum(),
        total_transmitted_bytes: interfaces.iter().map(|i| i.total_transmitted_bytes).sum(),
        interfaces,
    }
}
