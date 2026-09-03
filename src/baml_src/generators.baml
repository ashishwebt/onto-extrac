// Generator config for the Rust BAML client.
generator target {
    // Valid values: "python/pydantic", "typescript", "go", "rust", "ruby/sorbet", "rest/openapi"
    output_type "rust"

    // Where the generated code will be saved (relative to baml_src/)
    output_dir "../"

    // The version of the BAML package you have installed
    version "0.226.1"

    // Valid values: "sync", "async"
    default_client_mode sync
}
