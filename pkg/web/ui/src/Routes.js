import React from 'react';
import {
    Switch,
    Route,
  } from "react-router-dom";
import { Home } from './view/Home';
import { Pipes } from './view/Pipes';
import { Tunnels } from './view/Tunnels';

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