import React from 'react';
import ReactDOM from 'react-dom/client';
import { MantineProvider, createTheme } from '@mantine/core';
import { ModalsProvider } from '@mantine/modals';
import { Notifications } from '@mantine/notifications';
import '@mantine/core/styles.css';
import '@mantine/notifications/styles.css';
import '@xterm/xterm/css/xterm.css';
import './theme.css';
import App from './App';
import { ThemeBridge } from './ThemeBridge';

const theme = createTheme({
  fontFamily: 'Inter, Segoe UI, sans-serif',
  defaultRadius: 'md',
  primaryColor: 'blue',
  colors: {
    blue: [
      '#ddf4ff',
      '#b6e3ff',
      '#80ccff',
      '#54aeff',
      '#218bff',
      '#0969da',
      '#0550ae',
      '#033d8b',
      '#0a3069',
      '#002155',
    ],
    gray: [
      '#f6f8fa',
      '#eaeef2',
      '#d0d7de',
      '#afb8c1',
      '#8c959f',
      '#6e7781',
      '#57606a',
      '#424a53',
      '#32383f',
      '#24292f',
    ],
    dark: [
      '#e6edf3',
      '#c9d1d9',
      '#8b949e',
      '#6e7681',
      '#484f58',
      '#30363d',
      '#21262d',
      '#161b22',
      '#0d1117',
      '#010409',
    ],
  },
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
