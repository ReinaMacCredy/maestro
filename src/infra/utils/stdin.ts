/**
 * Read raw text from stdin for CLI pipeline support.
 * Use when commands accept --stdin to read content from a pipe.
 */
export async function readStdinText(): Promise<string> {
  return new Promise((resolve, reject) => {
    if (process.stdin.isTTY) {
      return reject(new Error('No stdin input (terminal is interactive). Pipe content or use --content instead.'));
    }
    let data = '';
    process.stdin.setEncoding('utf-8');
    process.stdin.on('data', (chunk: string) => { data += chunk; });
    process.stdin.on('end', () => resolve(data));
    process.stdin.on('error', reject);
    process.stdin.resume();
  });
}
