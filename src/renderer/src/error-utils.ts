export function getErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message) {
    return error.message
  }

  if (typeof error === 'string' && error.trim().length > 0) {
    return error
  }

  if (error && typeof error === 'object' && 'message' in error) {
    const message = (error as { message?: unknown }).message
    if (typeof message === 'string' && message.trim().length > 0) {
      return message
    }
  }

  try {
    const serialized = JSON.stringify(error)
    if (serialized && serialized !== '{}') {
      return serialized
    }
  } catch {
    // Ignore serialization failures and fall through to the default message.
  }

  return 'Unknown error'
}

export function toError(error: unknown): Error {
  return error instanceof Error ? error : new Error(getErrorMessage(error))
}
