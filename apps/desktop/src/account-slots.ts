import { useLayoutEffect, useState } from 'react';

// Keep provider controls in their account row without moving their async lifecycle.
export function useAccountSlot(id?: string) {
  const [element, setElement] = useState<HTMLElement | null>(null);
  useLayoutEffect(() => { setElement(id ? document.getElementById(id) : null); }, [id]);
  return element;
}
