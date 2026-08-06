import { useEffect } from 'react';
import { useStore } from './store/useStore';
import { Layout } from './components/Layout';

export default function App() {
  const { loadServers, loadGroups, loadCredentials, loadSettings, loadHistory, loadSshKeys } = useStore();

  useEffect(() => {
    loadServers();
    loadGroups();
    loadCredentials();
    loadSettings();
    loadHistory();
    loadSshKeys();
  }, []);

  return <Layout />;
}
