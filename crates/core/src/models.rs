//! Core data models: Device and Job

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceStatus {
    Disconnected,
    Connected,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Transport {
    Serial,
    Tcp,
    Udp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub port: String,
    pub baud: Option<u32>,
    pub firmware: Option<String>,
    pub capabilities: Vec<String>,
    pub status: DeviceStatus,
    pub transport: Transport,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobStatus {
    Queued,
    Running,
    Paused,
    Completed,
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub file_path: String,
    pub lines_total: usize,
    pub lines_sent: usize,
    pub progress: f32,
    pub status: JobStatus,
    pub created_at: DateTime<Utc>,
}
