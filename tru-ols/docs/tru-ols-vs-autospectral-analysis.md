**TRU-OLS** (Truncated ReUnmixing Ordinary Least Squares) and **AutoSpectral** are both computational approaches designed to improve the accuracy of spectral flow cytometry data, but they address different sources of error through distinct mathematical and pre-processing strategies. While TRU-OLS focuses on reducing variance (spread) by removing irrelevant fluorophores from the unmixing matrix on a per-cell basis 1, 2, AutoSpectral focuses on minimizing unmixing errors caused by cellular autofluorescence (AF) heterogeneity and spectral mismatches through iterative optimization and control refinement 3, 4\.

### Core Philosophy and Differences

**TRU-OLS** operates on the premise that including every possible dye in the unmixing matrix (M) for every cell increases the variance of the calculated abundances, particularly for dyes not actually present on the cell 1, 2\. Its primary goal is **feature selection**: determining which dyes are "irrelevant" for a specific event and removing them from the linear model to reduce spreading 5\.  
**AutoSpectral** aims to correct "unmixing errors" derived from insufficient statistical models and sample preparation issues, specifically targeting the mismatch between controls and fully stained samples 4\. It functions as a pipeline that refines the unmixing matrix itself by "cleaning" controls and then applying **per-cell autofluorescence extraction** and **fluorophore spectral optimization** based on residual minimization 6, 7\.  
The developers of AutoSpectral note that TRU-OLS deals with the propagation of noise driving spread, whereas AutoSpectral targets the underlying causes (AF and spectral mismatch), though the two methods are compatible 8\.

### Pre-processing and Control Refinement

The two methods employ significantly different steps prior to the final unmixing of the fully stained sample.  
**AutoSpectral** places heavy emphasis on "cleaning" the single-stained controls to generate an accurate initial spillover matrix:

* **Scatter-Matching:** To ensure the negative events in a control match the positive events in background characteristics, AutoSpectral selects the brightest positive events and defines a density-based boundary on their Forward and Side Scatter. It then applies this boundary to an unstained sample to select negative events with matching scatter profiles 9-11.  
* **Removal of Intrusive Events:** It identifies and removes "intrusive" AF events (e.g., macrophages or dead cells) from single-stained controls. This is done by scaling unstained data, performing Principal Component Analysis (PCA) to identify loadings driving variation, and using these to unmix the unstained sample into a 2D representation. A boundary is established to exclude these high-noise events from the calculation of the fluorophore spectrum 9, 12\.  
* **AF Signature Identification:** It identifies multiple AF signatures from an unstained sample using a self-organizing map (SOM) without metaclustering, treating the nodes as sources of potential AF signatures to be tested later 13\.

**TRU-OLS** relies on the unstained control primarily to establish statistical cutoffs for relevance:

* **Cutoff Determination:** The algorithm unmixes the unstained control using the full matrix. It then calculates the 99.5th percentile of the unmixed abundance distribution for each endmember (dye) 14\. This value serves as the threshold for determining if a dye is "relevant" or "irrelevant" on a specific cell in the fully stained sample 14, 15\.  
* **No Automatic Gating:** TRU-OLS does not determine biological relevance or perform gating; it uses the unstained control strictly to determine the noise floor for feature selection 16\.

### Linear Algebra and Unmixing Methods

The mathematical execution of the unmixing process is where the two approaches diverge most sharply.  
**TRU-OLS: The Stepwise Feature Selection Method**TRU-OLS modifies the **dimensions** of the unmixing matrix (M) for each event.

1. **Initial OLS:** The cell is unmixed using Ordinary Least Squares (OLS) with the full mixing matrix 14, 15\.  
2. **Relevance Check:** The calculated abundances are compared against the pre-determined 99.5th percentile cutoffs from the unstained control 14\.  
3. **Truncation:** If an abundance is below the cutoff, the endmember is deemed "irrelevant." Its abundance is set to zero, and the corresponding column is removed from the matrix M for that specific cell 15\. Autofluorescence is always considered relevant and never removed 17\.  
4. **Re-Unmixing:** The cell is unmixed again using the reduced (truncated) matrix 18\. This reduction in matrix columns reduces the standard error of the remaining coefficients, thereby tightening populations and reducing spread 19, 20\.

**AutoSpectral: The Residual Minimization Method**AutoSpectral modifies the **values** within the unmixing matrix and the AF vectors for each event, selecting the best fit from a library of options.

1. **Per-Cell AF Extraction:** The algorithm tests multiple AF signatures (derived from the SOM of the unstained sample) pairwise with optimized fluorophore signatures. It generates multiple OLS models for each cell and selects the AF signature that results in the **lowest squared residual** for that cell 13, 21\.  
2. **Per-Cell Fluorophore Optimization:** Similarly, AutoSpectral allows the spectral signature of the fluorophores to vary. It generates a SOM of positive control events to create a set of spectral variants. For each cell, it tests these variants and selects the spectrum that minimizes the residuals 7\.  
3. **Iterative Regression:** AutoSpectral employs Iterative Linear Regression (using OLS, Weighted Least Squares, or Iteratively Reweighted Least Squares) to minimize matrix error 6, 22\.  
4. **Detection Thresholding:** Similar to TRU-OLS, AutoSpectral checks if a cell has a detectable signal for a fluorophore (defined by the 99.5th percentile of the unstained sample). It re-unmixes using only detected fluorophores 7\. However, unlike TRU-OLS, AutoSpectral avoids discontinuities in visualization by allowing variants and fitting based on residual minimization rather than just dropping columns based on abundance cutoffs 23\.

### Visualization of Results

* **TRU-OLS:** By default, TRU-OLS sets irrelevant abundances to exactly zero. This can cause visualization issues where negative events collapse onto the axis or a single point (0,0) 24, 25\. To remedy this, TRU-OLS can map irrelevant abundances back to the distribution of the unstained control to create a natural-looking negative population for visualization purposes 26, 27\.  
* **AutoSpectral:** Encodes the selected AF signature and spectral variants into the FCS file, maintaining continuous distributions without the "bermuda region" (a gap between negative and positive populations) sometimes caused by the hard cutoffs in TRU-OLS 23, 28\.
