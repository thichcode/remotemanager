import { useEffect, useRef, useState } from 'react';
import { TextInput, Kbd, SegmentedControl, Group } from '@mantine/core';
import { IconSearch } from '@tabler/icons-react';
import { useStore } from '../store/useStore';
import type { Protocol } from '../types';

export function SearchBar() {
  const { searchServers, loadServers, setSearchQuery } = useStore();
  const [value, setValue] = useState('');
  const [protocol, setProtocol] = useState<string>('all');
  const inputRef = useRef<HTMLInputElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        inputRef.current?.focus();
        inputRef.current?.select();
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => {
      window.removeEventListener('keydown', onKeyDown);
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, []);

  const handleChange = (newValue: string) => {
    setValue(newValue);
    setSearchQuery(newValue.trim());
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => {
      searchServers(newValue);
    }, 250);
  };

  const handleProtocolChange = (val: string) => {
    setProtocol(val);
    if (val === 'all') {
      if (!value.trim()) loadServers();
    }
    window.dispatchEvent(new CustomEvent('rm:filter-protocol', { detail: val as Protocol | 'all' }));
  };

  return (
    <Group gap="sm">
      <SegmentedControl
        size="xs"
        value={protocol}
        onChange={handleProtocolChange}
        data={[
          { label: 'All', value: 'all' },
          { label: 'SSH', value: 'ssh' },
          { label: 'RDP', value: 'rdp' },
        ]}
      />
      <TextInput
        placeholder="Search servers..."
        leftSection={<IconSearch size={14} />}
        rightSection={<Kbd size="xs">Ctrl+K</Kbd>}
        value={value}
        onChange={(e) => handleChange(e.currentTarget.value)}
        ref={inputRef}
        w={280}
        size="sm"
      />
    </Group>
  );
}
