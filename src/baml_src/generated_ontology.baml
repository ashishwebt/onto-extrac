class ExtractionResult {
  status bool @description(#"
    Extraction status
  "#)
  @@dynamic
}

function ExtractInfo(input: string) -> ExtractionResult {
  client "CustomGemini"
  prompt #"
    Extract structured information from the following text based on the requested schema.

    {{ ctx.output_format }}

    {{ _.role("user") }} {{ input }}
  "#
}
