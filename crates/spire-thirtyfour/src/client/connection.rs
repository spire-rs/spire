//! Browser connection with enhanced metadata and lifecycle tracking.

use std::fmt;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use derive_more::Display;
use thirtyfour::WebDriver;

use crate::config::WebDriverConfig;

/// Represents a connection to a browser instance with enhanced tracking.
///
/// This type wraps a Thirtyfour WebDriver along with comprehensive metadata
/// for connection lifecycle management, health monitoring, and usage statistics.
/// It provides direct access to the underlying WebDriver through `Deref` traits
/// while maintaining connection state information.
///
/// # Examples
///
/// ```ignore
/// use spire_thirtyfour::client::connection::BrowserConnection;
/// use thirtyfour::WebDriver;
///
/// let driver = WebDriver::new("http://localhost:4444", capabilities).await?;
/// let config = WebDriverConfig::new("http://localhost:4444");
/// let connection = BrowserConnection::new(1, driver, config);
///
/// // Use as WebDriver directly
/// connection.goto("https://example.com").await?;
///
/// // Access metadata
/// println!("Connection {} created at {:?}", connection.id(), connection.created_at());
/// ```
pub struct BrowserConnection {
    /// Unique identifier for this connection
    pub(crate) id: u64,

    /// The underlying Thirtyfour WebDriver client
    pub(crate) client: WebDriver,

    /// Configuration used to create this connection
    pub(crate) config: WebDriverConfig,

    /// Timestamp when this connection was created
    pub(crate) created_at: Instant,

    /// Timestamp when this connection was last used
    pub(crate) last_used: Instant,

    /// Number of requests processed by this connection
    request_count: Arc<AtomicU64>,

    /// Number of errors encountered by this connection
    error_count: Arc<AtomicU64>,

    /// Current health status of the connection
    healthy: Arc<std::sync::atomic::AtomicBool>,

    /// Session ID from the WebDriver (if available)
    session_id: Option<String>,
}

impl BrowserConnection {
    /// Creates a new browser connection.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for this connection
    /// * `client` - The WebDriver instance
    /// * `config` - Configuration used to create the WebDriver
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let connection = BrowserConnection::new(1, webdriver, config);
    /// ```
    pub fn new(id: u64, client: WebDriver, config: WebDriverConfig) -> Self {
        let now = Instant::now();

        // Try to get the session ID from the WebDriver
        let session_id = Some(client.session_id().to_string());

        Self {
            id,
            client,
            config,
            created_at: now,
            last_used: now,
            request_count: Arc::new(AtomicU64::new(0)),
            error_count: Arc::new(AtomicU64::new(0)),
            healthy: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            session_id,
        }
    }

    /// Returns the unique identifier for this connection.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Returns the configuration used to create this connection.
    pub fn config(&self) -> &WebDriverConfig {
        &self.config
    }

    /// Returns when this connection was created.
    pub fn created_at(&self) -> Instant {
        self.created_at
    }

    /// Returns when this connection was last used.
    pub fn last_used(&self) -> Instant {
        self.last_used
    }

    /// Returns the WebDriver session ID if available.
    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    /// Returns the age of this connection.
    pub fn age(&self) -> std::time::Duration {
        self.created_at.elapsed()
    }

    /// Returns how long since this connection was last used.
    pub fn idle_time(&self) -> std::time::Duration {
        self.last_used.elapsed()
    }

    /// Returns the number of requests processed by this connection.
    pub fn request_count(&self) -> u64 {
        self.request_count.load(Ordering::Relaxed)
    }

    /// Returns the number of errors encountered by this connection.
    pub fn error_count(&self) -> u64 {
        self.error_count.load(Ordering::Relaxed)
    }

    /// Returns whether this connection is considered healthy.
    pub fn is_healthy(&self) -> bool {
        self.healthy.load(Ordering::Relaxed)
    }

    /// Marks this connection as used (updates last_used timestamp).
    pub fn mark_used(&mut self) {
        self.last_used = Instant::now();
        self.request_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Marks this connection as having encountered an error.
    pub fn mark_error(&self) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Sets the health status of this connection.
    pub fn set_healthy(&self, healthy: bool) {
        self.healthy.store(healthy, Ordering::Relaxed);
    }

    /// Returns connection statistics.
    pub fn stats(&self) -> ConnectionStats {
        ConnectionStats {
            id: self.id,
            age: self.age(),
            idle_time: self.idle_time(),
            request_count: self.request_count(),
            error_count: self.error_count(),
            healthy: self.is_healthy(),
            session_id: self.session_id.clone(),
            config_url: self.config.url.clone(),
        }
    }

    /// Performs a basic health check on this connection.
    ///
    /// This is a lightweight check that doesn't make network calls.
    /// For comprehensive health checks, use the manager's health check methods.
    pub fn quick_health_check(&self) -> bool {
        // Check if marked unhealthy
        if !self.is_healthy() {
            return false;
        }

        // Check error rate
        let requests = self.request_count();
        let errors = self.error_count();

        if requests > 0 {
            let error_rate = errors as f64 / requests as f64;
            if error_rate > 0.5 {
                // More than 50% error rate indicates unhealthy connection
                return false;
            }
        }

        true
    }

    /// Resets the connection statistics.
    pub fn reset_stats(&self) {
        self.request_count.store(0, Ordering::Relaxed);
        self.error_count.store(0, Ordering::Relaxed);
        self.healthy.store(true, Ordering::Relaxed);
    }

    /// Returns a reference to the underlying WebDriver.
    pub fn webdriver(&self) -> &WebDriver {
        &self.client
    }

    /// Returns a mutable reference to the underlying WebDriver.
    pub fn webdriver_mut(&mut self) -> &mut WebDriver {
        &mut self.client
    }
}

impl Deref for BrowserConnection {
    type Target = WebDriver;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

impl DerefMut for BrowserConnection {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.client
    }
}

impl fmt::Debug for BrowserConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrowserConnection")
            .field("id", &self.id)
            .field("config_url", &self.config.url)
            .field("created_at", &self.created_at)
            .field("last_used", &self.last_used)
            .field("request_count", &self.request_count())
            .field("error_count", &self.error_count())
            .field("healthy", &self.is_healthy())
            .field("session_id", &self.session_id)
            .finish_non_exhaustive()
    }
}

impl fmt::Display for BrowserConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "BrowserConnection(id={}, url={}, age={:.1}s, requests={}, healthy={})",
            self.id,
            self.config.url,
            self.age().as_secs_f64(),
            self.request_count(),
            self.is_healthy()
        )
    }
}

/// Statistics for a browser connection.
#[derive(Debug, Clone, PartialEq, Display)]
#[display(
    fmt = "Stats(id={}, age={:.1}s, idle={:.1}s, req={}, err={}, healthy={})",
    id,
    "age.as_secs_f64()",
    "idle_time.as_secs_f64()",
    request_count,
    error_count,
    healthy
)]
pub struct ConnectionStats {
    /// Connection ID
    pub id: u64,
    /// Age of the connection
    pub age: std::time::Duration,
    /// Time since last use
    pub idle_time: std::time::Duration,
    /// Number of requests processed
    pub request_count: u64,
    /// Number of errors encountered
    pub error_count: u64,
    /// Health status
    pub healthy: bool,
    /// WebDriver session ID
    pub session_id: Option<String>,
    /// Configuration URL
    pub config_url: String,
}

impl ConnectionStats {
    /// Returns the error rate for this connection.
    pub fn error_rate(&self) -> f64 {
        if self.request_count == 0 {
            0.0
        } else {
            self.error_count as f64 / self.request_count as f64
        }
    }

    /// Returns whether this connection has high error rate.
    pub fn has_high_error_rate(&self, threshold: f64) -> bool {
        self.error_rate() > threshold
    }

    /// Returns whether this connection is considered stale.
    pub fn is_stale(&self, max_age: std::time::Duration, max_idle: std::time::Duration) -> bool {
        self.age > max_age || self.idle_time > max_idle
    }
}

/// A collection of connection statistics for analysis.
#[derive(Debug, Default, Display)]
#[display(
    fmt = "ConnectionStatsCollection(count={}, healthy={})",
    "stats.len()",
    "stats.iter().filter(|s| s.healthy).count()"
)]
pub struct ConnectionStatsCollection {
    stats: Vec<ConnectionStats>,
}

impl ConnectionStatsCollection {
    /// Creates a new empty collection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds statistics to the collection.
    pub fn add(&mut self, stats: ConnectionStats) {
        self.stats.push(stats);
    }

    /// Returns all statistics in the collection.
    pub fn all(&self) -> &[ConnectionStats] {
        &self.stats
    }

    /// Returns the number of connections in the collection.
    pub fn count(&self) -> usize {
        self.stats.len()
    }

    /// Returns the number of healthy connections.
    pub fn healthy_count(&self) -> usize {
        self.stats.iter().filter(|s| s.healthy).count()
    }

    /// Returns the number of unhealthy connections.
    pub fn unhealthy_count(&self) -> usize {
        self.count() - self.healthy_count()
    }

    /// Returns the total number of requests across all connections.
    pub fn total_requests(&self) -> u64 {
        self.stats.iter().map(|s| s.request_count).sum()
    }

    /// Returns the total number of errors across all connections.
    pub fn total_errors(&self) -> u64 {
        self.stats.iter().map(|s| s.error_count).sum()
    }

    /// Returns the overall error rate across all connections.
    pub fn overall_error_rate(&self) -> f64 {
        let total_requests = self.total_requests();
        if total_requests == 0 {
            0.0
        } else {
            self.total_errors() as f64 / total_requests as f64
        }
    }

    /// Returns the average age of connections.
    pub fn average_age(&self) -> std::time::Duration {
        if self.stats.is_empty() {
            return std::time::Duration::ZERO;
        }

        let total_secs: f64 = self.stats.iter().map(|s| s.age.as_secs_f64()).sum();

        std::time::Duration::from_secs_f64(total_secs / self.stats.len() as f64)
    }

    /// Returns connections that meet the specified criteria.
    pub fn filter<F>(&self, predicate: F) -> Vec<&ConnectionStats>
    where
        F: Fn(&ConnectionStats) -> bool,
    {
        self.stats.iter().filter(|s| predicate(s)).collect()
    }

    /// Returns connections with high error rates.
    pub fn high_error_connections(&self, threshold: f64) -> Vec<&ConnectionStats> {
        self.filter(|s| s.has_high_error_rate(threshold))
    }

    /// Returns stale connections.
    pub fn stale_connections(
        &self,
        max_age: std::time::Duration,
        max_idle: std::time::Duration,
    ) -> Vec<&ConnectionStats> {
        self.filter(|s| s.is_stale(max_age, max_idle))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn connection_stats_error_rate() {
        let stats = ConnectionStats {
            id: 1,
            age: Duration::from_secs(100),
            idle_time: Duration::from_secs(10),
            request_count: 10,
            error_count: 2,
            healthy: true,
            session_id: None,
            config_url: "http://localhost:4444".to_string(),
        };

        assert_eq!(stats.error_rate(), 0.2);
        assert!(!stats.has_high_error_rate(0.5));
        assert!(stats.has_high_error_rate(0.1));
    }

    #[test]
    fn connection_stats_stale_detection() {
        let stats = ConnectionStats {
            id: 1,
            age: Duration::from_secs(3600),      // 1 hour
            idle_time: Duration::from_secs(600), // 10 minutes
            request_count: 100,
            error_count: 5,
            healthy: true,
            session_id: None,
            config_url: "http://localhost:4444".to_string(),
        };

        // Not stale with generous limits
        assert!(!stats.is_stale(Duration::from_secs(7200), Duration::from_secs(1200)));

        // Stale due to age
        assert!(stats.is_stale(Duration::from_secs(1800), Duration::from_secs(1200)));

        // Stale due to idle time
        assert!(stats.is_stale(Duration::from_secs(7200), Duration::from_secs(300)));
    }

    #[test]
    fn stats_collection_aggregation() {
        let mut collection = ConnectionStatsCollection::new();

        collection.add(ConnectionStats {
            id: 1,
            age: Duration::from_secs(100),
            idle_time: Duration::from_secs(10),
            request_count: 10,
            error_count: 1,
            healthy: true,
            session_id: None,
            config_url: "http://localhost:4444".to_string(),
        });

        collection.add(ConnectionStats {
            id: 2,
            age: Duration::from_secs(200),
            idle_time: Duration::from_secs(20),
            request_count: 20,
            error_count: 4,
            healthy: false,
            session_id: None,
            config_url: "http://localhost:4445".to_string(),
        });

        assert_eq!(collection.count(), 2);
        assert_eq!(collection.healthy_count(), 1);
        assert_eq!(collection.unhealthy_count(), 1);
        assert_eq!(collection.total_requests(), 30);
        assert_eq!(collection.total_errors(), 5);
        assert_eq!(collection.overall_error_rate(), 5.0 / 30.0);

        // Average age should be (100 + 200) / 2 = 150 seconds
        let avg_age = collection.average_age();
        assert_eq!(avg_age, Duration::from_secs(150));
    }

    #[test]
    fn stats_collection_filtering() {
        let mut collection = ConnectionStatsCollection::new();

        // Add healthy connection
        collection.add(ConnectionStats {
            id: 1,
            age: Duration::from_secs(100),
            idle_time: Duration::from_secs(10),
            request_count: 100,
            error_count: 5, // 5% error rate
            healthy: true,
            session_id: None,
            config_url: "http://localhost:4444".to_string(),
        });

        // Add connection with high error rate
        collection.add(ConnectionStats {
            id: 2,
            age: Duration::from_secs(200),
            idle_time: Duration::from_secs(20),
            request_count: 10,
            error_count: 6, // 60% error rate
            healthy: true,
            session_id: None,
            config_url: "http://localhost:4445".to_string(),
        });

        let high_error = collection.high_error_connections(0.1);
        assert_eq!(high_error.len(), 1);
        assert_eq!(high_error[0].id, 2);
    }
}
