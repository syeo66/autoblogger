# Autoblogger

Autoblogger is a simple blogging platform that generates blog posts using the ChatGPT language model. Please note that the generated content is based on AI-generated responses and may not always be accurate, reliable, or factual. The code provided here is for demonstration purposes only and comes with the following warnings and limitations:

- **Hallucination**: The AI model used by Autoblogger may sometimes generate text that is creative but not necessarily grounded in reality. The generated content should be carefully reviewed and fact-checked before publishing.

- **Inaccuracies**: The AI model may produce incorrect or misleading information. It is essential to verify the generated content and ensure its accuracy before relying on it.

- **Costs**: The Autoblogger code interacts with paid APIs. This will incur costs depending on your usage. Make sure to review the pricing and terms of service of the API in use before deploying this code in production or on public servers.

- **Not suitable for public servers**: Due to the potential inaccuracies and hallucinations in the generated content, it is strongly recommended not to use Autoblogger on public servers or platforms where the generated content is accessible to a wide audience. It is more suitable for personal use or controlled environments where the content can be reviewed and verified before publication.

Please exercise caution and use this code responsibly. OpenAI's guidelines and best practices should be followed when deploying AI-generated content.

## Installation

1. Clone the repository:

   ```shell
   git clone https://github.com/syeo66/autoblogger.git
   ```

2. Navigate to the project directory:

   ```shell
   cd autoblogger
   ```

3. Install the dependencies:

   ```shell
   cargo build
   ```

## Usage

1. Set the `AI_MODEL` environment variable to `gpt4`, `claude3` or `claude4`

2. Set the `OPENAI_API_KEY` environment variable with your OpenAI API key,
   or use `ANTHROPIC_API_KEY` if you use the Claude model:

   ```shell
   export OPENAI_API_KEY=your-api-key
   ```

   or

   ```shell
   export ANTHROPIC_API_KEY=your-api-key
   ```

3. Start the autoblogger server:

   ```shell
   cargo run
   ```

3. Access the autoblogger web interface by opening `http://localhost:3000` in your web browser.

4. Create a blog post by opening `http://localhot:3000/<some-slug-describing-the-article-to-be-generated>` 

5. The generated blog post will be displayed on the webpage and stored in the `./blog.db` SQLite database.

## License

This project is licensed under the [MIT License](LICENSE).

