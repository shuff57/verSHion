//! Parallel lines cut by transversals, as an editor: up to six parallel lines and four transversals, each line clicked on the figure
//! to style, rename or delete it, angles clicked for congruence arcs or shading, and named points put where lines cross or at a
//! line's end. Every addition is a new param (empty or at a default that draws nothing new), so a figure saved or shared before
//! them draws exactly what it did.

#[cfg(test)]
mod frozen;
