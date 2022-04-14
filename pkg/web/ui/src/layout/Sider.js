import React, { useState } from 'react';

import { Layout, Menu } from 'antd';
import {
  FilterOutlined,
  HomeOutlined
} from '@ant-design/icons';

import {
  Link,
  useLocation
} from "react-router-dom";
import { Routes } from '../Routes';

const { Header, Content, Sider } = Layout;

export const SiderLayout = () => {
  const [collapsed, setCollapsed] = useState(false);
  
  const location = useLocation();

  const logoStyle = {
    color: "white",
    fontSize: 20,
    fontWeight: "bold",
    paddingLeft: 20
  }
  let title = "🐸 Rospo"
  if (collapsed) {
    title = "🐸"
  }
  return (
    <Layout style={{ minHeight: '100vh' }}>
      <Sider collapsible collapsed={collapsed} onCollapse={() => setCollapsed(!collapsed)}>
        <Header className="site-layout-background" style={{ padding: 0 }}>
          <div style={logoStyle}>{title}</div>
        </Header>
        <Menu theme="dark" 
                defaultSelectedKeys={['/']}
                selectedKeys={[location.pathname]}
                mode="inline">
          <Menu.Item key="/" icon={<HomeOutlined />}>
              <Link to="/">Home</Link>
          </Menu.Item>
          <Menu.Item key="/tunnels" icon={<FilterOutlined />}>
              <Link to="/tunnels">Tunnels</Link>
          </Menu.Item>
        </Menu>
      </Sider>
      <Layout className="site-layout">
        <Header className="site-layout-background" style={{ padding: 0 }} />
        <Content style={{ margin: '0 16px' }}>
          <div className="site-layout-background" style={{ padding: 24, minHeight: 360 }}>
            <Routes />
          </div>
        </Content>
      </Layout>
    </Layout>
  )
}