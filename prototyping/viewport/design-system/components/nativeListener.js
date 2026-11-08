export function nativeListener(type, handler) {
  return (element) => {
    element.addEventListener(type, handler);
    return () => element.removeEventListener(type, handler);
  };
}
