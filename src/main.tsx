import React from 'react';
import ReactDOM from 'react-dom/client';
import { MantineProvider, createTheme } from '@mantine/core';
import { ModalsProvider } from '@mantine/modals';
import { Notifications } from '@mantine/notifications';
import '@mantine/core/styles.css';
import '@mantine/notifications/styles.css';
import App from './App';
import { ThemeBridge } from './ThemeBridge';

const theme = createTheme({
  fontFamily: 'Inter, Segoe UI, sans-serif',
  defaultRadius: 'md',
});

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <MantineProvider theme={theme} defaultColorScheme="dark">
      <ThemeBridge>
        <ModalsProvider>
          <Notifications />
          <App />
        </ModalsProvider>
      </ThemeBridge>
    </MantineProvider>
  </React.StrictMode>
);
