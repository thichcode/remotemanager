import { useEffect } from 'react';
import { useMantineColorScheme } from '@mantine/core';
import { useStore } from './store/useStore';

export function ThemeBridge({ children }: { children: React.ReactNode }) {
  const settings = useStore((s) => s.settings);
  const { setColorScheme } = useMantineColorScheme();

  useEffect(() => {
    if (settings) {
      setColorScheme(settings.theme);
    }
  }, [settings?.theme]);

  return <>{children}</>;
}
