# vector-fields.rs

A visualization of vector fields in Rust using Tetra.
Spawns and renders a bunch of particles on the screen.

## Installation and running

Clone this repository, then navigate into it and run the code:

```sh
git clone https://github.com/adri326/vector-fields.rs vector-fields
cd vector-fields
cargo run --release
```

## Modifying the parameters and the function

A lot of the parameters can be changed, have a look at the many constants in `src/main.rs`!

You can also change the function that governs the vector field: edit the contents of the function `f` to your heart's content!
Currently, `f` is a fractal, defined as such:

```
f_i(z) = z + z^i*e^-i

f(z) = f_n(f_{n-1}(...(f_2(z))...))
```
