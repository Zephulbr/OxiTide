/// Flip a value optimistically, persist, and revert on failure. Kept as a
/// standalone helper so settings handlers share one rollback shape and the
/// behavior is testable without component infra.
export async function optimistic<T>(
  value: T,
  next: T,
  set: (value: T) => void,
  persist: () => Promise<void>,
  onError?: (error: unknown) => void,
): Promise<void> {
  const previous = value;
  set(next);
  try {
    await persist();
  } catch (error) {
    set(previous);
    onError?.(error);
  }
}
