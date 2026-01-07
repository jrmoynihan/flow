pub mod margins;
pub mod doublets;
pub mod peaks;
pub mod mad;
pub mod consecutive;
pub mod isolation_tree;
pub mod monotonic;
pub mod peacoqc;

pub use margins::{remove_margins, MarginConfig, MarginResult};
pub use doublets::{remove_doublets, DoubletConfig, DoubletResult};
pub use peaks::{determine_peaks_all_channels, PeakDetectionConfig, ChannelPeakFrame, PeakInfo};
pub use mad::{mad_outlier_method, MADConfig, MADResult};
pub use consecutive::{remove_short_regions, ConsecutiveConfig};
pub use isolation_tree::{isolation_tree_detect, IsolationTreeConfig, IsolationTreeResult};
pub use monotonic::{find_increasing_decreasing_channels, MonotonicConfig, MonotonicResult};
pub use peacoqc::{peacoqc, PeacoQCConfig, PeacoQCResult, QCMode};
