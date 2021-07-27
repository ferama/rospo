import React from 'react';
import {
    Switch,
    Route,
  } from "react-router-dom";
import { Home } from './component/Home';
import { Pipes } from './component/Pipes';
import { Tunnels } from './component/Tunnels';

export const Routes = () => (
    <Switch>
        <Route path="/pipes">
            <Pipes />
        </Route>
        <Route path="/tunnels">
            <Tunnels />
        </Route>
        <Route path="/">
            <Home />
        </Route>
    </Switch>
)