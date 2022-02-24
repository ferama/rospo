import React from 'react';
import {
    Switch,
    Route,
  } from "react-router-dom";
import { Home } from './view/Home';
import { Tunnels } from './view/Tunnels';

export const Routes = () => (
    <Switch>
        <Route path="/tunnels">
            <Tunnels />
        </Route>
        <Route path="/">
            <Home />
        </Route>
    </Switch>
)