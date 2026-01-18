pub mod consecutive;
pub mod debug;
pub mod doublets;
pub mod export;
pub mod export;
pub mod isolation_tree;
pub mod mad;
pub mod margins;
pub mod monotonic;
pub mod peacoqc;
pub mod plots;

pub use consecutive::{ConsecutiveConfig, remove_short_regions};
pub use doublets::{DoubletConfig, DoubletResult, remove_doublets};
pub use export::{
    QCExportFormat, QCExportOptions, export_csv_boolean, export_csv_numeric, export_json_metadata,
};
pub use isolation_tree::{IsolationTreeConfig, IsolationTreeResult, isolation_tree_detect};
pub use mad::{MADConfig, MADResult, mad_outlier_method};
pub use margins::{MarginConfig, MarginResult, remove_margins};
pub use monotonic::{MonotonicConfig, MonotonicResult, find_increasing_decreasing_channels};
pub use peacoqc::{PeacoQCConfig, PeacoQCResult, QCMode, peacoqc};
pub use peaks::{ChannelPeakFrame, PeakDetectionConfig, PeakInfo, determine_peaks_all_channels};
pub use plots::{QCPlotConfig, create_qc_plots};
