# instplot-layout

Production single-axes layout compiler for InstPlot Studio. It owns scale
transforms, major/minor tick location, formatting, bounded layout, publication
artists, deterministic legend placement and the hit map. Preview and export
backends consume its one point-space Display List and do not relayout content.

The implementation was promoted from the accepted A7 spike. The historical
`layout-engine-spike` package is now a compatibility facade that runs the same
A7 tests and examples against this crate.
