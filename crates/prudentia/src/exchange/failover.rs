//! Failover manager for exchange outages and health monitoring

/// Configuration for exchange failover behavior
#[derive(Debug, Clone)]
pub struct ExchangeFailoverConfig {
    /// Primary exchange name
    pub primary_exchange: String,
    /// Backup exchanges in priority order
    pub backup_exchanges: Vec<String>,
    /// Health check interval in seconds
    pub health_check_interval_secs: u64,
}

/// Manages exchange failover and health monitoring
#[derive(Debug)]
pub struct FailoverManager {
    /// Failover configuration
    config: ExchangeFailoverConfig,
    /// Current primary exchange (may differ from config if failed over)
    current_primary: String,
}

impl FailoverManager {
    /// Create a new failover manager
    pub fn new(config: ExchangeFailoverConfig) -> Self {
        let current_primary = config.primary_exchange.clone();
        Self {
            config,
            current_primary,
        }
    }
    
    /// Get the current primary exchange name
    pub fn get_primary_exchange_name(&self) -> String {
        self.current_primary.clone()
    }
    
    /// Trigger failover to next available exchange
    pub fn failover_to_next(&mut self) -> Option<String> {
        // If we're currently on the primary, try first backup
        if self.current_primary == self.config.primary_exchange {
            if let Some(first_backup) = self.config.backup_exchanges.first() {
                self.current_primary = first_backup.clone();
                return Some(self.current_primary.clone());
            }
        } else {
            // Find current position in backup list and move to next
            if let Some(current_index) = self.config.backup_exchanges.iter()
                .position(|x| x == &self.current_primary) {
                if current_index + 1 < self.config.backup_exchanges.len() {
                    self.current_primary = self.config.backup_exchanges[current_index + 1].clone();
                    return Some(self.current_primary.clone());
                }
            }
        }
        None // No more backups available
    }
    
    /// Reset to primary exchange (called when primary is healthy again)
    pub fn reset_to_primary(&mut self) {
        self.current_primary = self.config.primary_exchange.clone();
    }
}