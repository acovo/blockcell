export interface SocketLike {
  readyState: number;
  send(value: string): void;
}

export function forCurrentSocket<T extends unknown[]>(
  current: () => unknown,
  socket: unknown,
  callback: (...args: T) => void,
) {
  return (...args: T) => {
    if (current() === socket) callback(...args);
  };
}

export function sendJson(socket: SocketLike | null, data: unknown, openState = 1): boolean {
  if (!socket || socket.readyState !== openState) return false;
  socket.send(JSON.stringify(data));
  return true;
}
