# TRU-OLS documentation and notes

This directory holds design notes, validation reports, and analysis documents for the `flow-tru-ols` crate.

| Document | Description |
|----------|-------------|
| [dev-notes.md](dev-notes.md) | Development notes: future enhancements, mixing matrix sources (CSV, SPILL, single-stain controls). |
| [validation-report.md](validation-report.md) | Algorithm validation vs Julia implementation; fixes for autofluorescence and overdetermined solve. |
| [comparison-with-julia.md](comparison-with-julia.md) | Comparison framework and step-by-step validation (preprocessing, unmixing, UCM). |
| [peak-detection-validation.md](peak-detection-validation.md) | Peak detection for primary-detector identification; validation on filtered vs unfiltered controls. |
| [unmixing-results-plate001.md](unmixing-results-plate001.md) | Plate_001 unmixing run (6 donors, UCM strategy, peak detection). |
| [tru-ols-vs-autospectral-analysis.md](tru-ols-vs-autospectral-analysis.md) | TRU-OLS vs AutoSpectral: philosophy, preprocessing, linear algebra, visualization. |
| [synergy-autospectral-tru-ols.md](synergy-autospectral-tru-ols.md) | Using AutoSpectral and TRU-OLS together in a single pipeline. |
