# Autoblogger

Autoblogger is a simple blogging platform that generates blog posts using the ChatGPT language model. It allows you to create blog posts based on a slug.

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

1. Set the `OPENAI_API_KEY` environment variable with your OpenAI API key:

   ```shell
   export OPENAI_API_KEY=your-api-key
   ```

2. Start the autoblogger server:

   ```shell
   cargo run
   ```

3. Access the autoblogger web interface by opening `http://localhost:3000` in your web browser.

4. Create a blog post by opening `http://localhot:3000/<some-slug-describing-the-article-to-be-generated>` 

5. The generated blog post will be displayed on the webpage and stored in the `./blog.db` SQLite database.

## License

This project is licensed under the [MIT License](LICENSE).

