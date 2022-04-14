import React from 'react';
import {
    Routes as RouterRoutes,
    Route,
  } from "react-router-dom";
import { Home } from './view/Home';
import { Tunnels } from './view/Tunnels';

export const Routes = () => (
    <RouterRoutes>
        <Route path="/tunnels" element={<Tunnels />} />
        <Route path="/" element={<Home />} />
    </RouterRoutes>
)