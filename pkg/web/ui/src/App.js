import React from 'react';
import './App.css';
import {
  // BrowserRouter as Router,
  HashRouter as Router
} from "react-router-dom";

import { SiderLayoutWithRouter } from './layout/Sider'

const App = () => (
  <Router>
    <SiderLayoutWithRouter />
  </Router>
);

export default App;