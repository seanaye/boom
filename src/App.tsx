import { Router, Routes, Route, hashIntegration } from "@solidjs/router";
import { lazy } from "solid-js";

const Index = lazy(() => import("./pages/Index"));
const Screenshot = lazy(() => import("./pages/Screenshot"));
const Settings = lazy(() => import("./pages/Settings"));

export default function App() {
  return (
    <Router source={hashIntegration()}>
      <Routes>
        <Route path="/" component={Index} />
        <Route path="/screenshot" component={Screenshot} />
        <Route path="/settings" component={Settings} />
      </Routes>
    </Router>
  );
}
