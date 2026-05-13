/**
 * Exhaustiveness check for discriminated unions.
 * 
 * Use in the default case of a switch statement to ensure all cases are handled.
 * If a new case is added to the union type, TypeScript will error at compile time.
 * 
 * @example
 * ```ts
 * type Status = "pending" | "success" | "error";
 * 
 * function handleStatus(status: Status): string {
 *   switch (status) {
 *     case "pending": return "Loading...";
 *     case "success": return "Done!";
 *     case "error": return "Failed!";
 *     default: return assertNever(status);
 *   }
 * }
 * ```
 */
export function assertNever(value: never): never {
  throw new Error(`Unexpected value: ${JSON.stringify(value)}`);
}
