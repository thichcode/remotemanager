import { useState } from 'react';
import { TextInput, Kbd } from '@mantine/core';
import { IconSearch } from '@tabler/icons-react';
import { useStore } from '../store/useStore';

export function SearchBar() {
  const { searchServers } = useStore();
  const [value, setValue] = useState('');

  const handleChange = (newValue: string) => {
    setValue(newValue);
    searchServers(newValue);
  };

  return (
    <TextInput
      placeholder="Search servers..."
      leftSection={<IconSearch size={14} />}
      rightSection={<Kbd size="xs">Ctrl+K</Kbd>}
      value={value}
      onChange={(e) => handleChange(e.currentTarget.value)}
      w={300}
      size="sm"
    />
  );
}
