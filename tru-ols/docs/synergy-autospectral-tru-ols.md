Yes, **AutoSpectral and TRU-OLS can be used together in a single pipeline.** The developers of AutoSpectral explicitly state that while the two methods are distinct in their approach, they are compatible and TRU-OLS "could be integrated" into the AutoSpectral workflow 1\.  
They can be combined effectively because they address different mathematical aspects of the unmixing problem:

* **AutoSpectral optimizes the *values* within the matrix:** It focuses on correcting the "underlying causes" of error—specifically cellular autofluorescence and spectral mismatch—by refining the spectral signatures and autofluorescence vectors used in the mixing matrix (M) on a per-cell basis 1, 2\.  
* **TRU-OLS optimizes the *dimensions* of the matrix:** It focuses on mitigating the "propagation of noise" that drives spread. It does this by identifying which dyes are biologically irrelevant for a specific cell and removing those columns from the mixing matrix entirely 1, 3\.

### How a Combined Pipeline Would Work

In a theoretical combined pipeline, the two methods would likely function sequentially to minimize both bias and variance:

1. **Refining the Inputs (AutoSpectral):** First, the pipeline would use AutoSpectral to generate the most accurate possible mixing matrix for each cell. This involves "cleaning" the controls to get accurate spectral signatures and selecting the best-fit autofluorescence and fluorophore variants for that specific event 4, 5\.  
2. **Feature Selection (TRU-OLS):** Once the optimal matrix is defined by AutoSpectral, the pipeline would apply the TRU-OLS logic. It would perform an initial unmixing to determine which fluorophores have abundances below the noise threshold (determined from the AutoSpectral-cleaned unstained control) 6, 7\.  
3. **Final Unmixing (Combined):** The columns corresponding to these "irrelevant" dyes would be removed (truncated) from the AutoSpectral-optimized matrix. The cell would then be re-unmixed using this reduced, highly accurate matrix, resulting in data with minimized spectral error (from AutoSpectral) and minimized spread/variance (from TRU-OLS) 1, 8, 9\.
