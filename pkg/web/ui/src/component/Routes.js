import React from 'react';
import {
    Switch,
    Route,
  } from "react-router-dom";
import { Pipes } from './Pipes';
import { Tunnels } from './Tunnels';

export const Routes = () => (
    <Switch>
        <Route path="/pipes">
            <Pipes />
        </Route>
        <Route path="/">
            <Tunnels />
        </Route>
    </Switch>
)