import React from 'react';
import './App.css';
import {
  // BrowserRouter as Router,
  HashRouter as Router
} from "react-router-dom";

import { SiderLayout } from './layout/Sider'

const App = () => (
  <Router>
    <SiderLayout />
  </Router>
);

export default App;