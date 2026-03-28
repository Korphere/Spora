# USAGE

## fetch

`spora fetch`

Download and setup the required JDK and dependencies defined in your configuration. It ensures your environment is ready for building.

## build

`spora build`

Compile your project using the managed toolchain. Spora automatically handles the compiler paths and flags for you.

## bloom (fetch and build)

`spora bloom`

The recommended workflow. It runs fetch and build in one go. Just like a spore blooming, it brings your project to life from scratch.

## clean

`spora clean`

Remove build artifacts and temporary files to start fresh.

## init

`spora init`

Initialize a new Spora project in the current directory. This creates a default spora.toml to get you started.

## run

`spora run`

Build (if necessary) and execute your application or plugin.

## Example: Start a new project and run it immediately

spora init
spora bloom
spora run
