client<llm> CustomGemini {
  provider google-ai
  options {
    model "gemini-3.6-flash"
    api_key env.GOOGLE_API_KEY
  }
}
