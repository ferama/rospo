import React from 'react';

import { Layout, Menu } from 'antd';
import {
  FunnelPlotOutlined,
  FilterOutlined,
} from '@ant-design/icons';

import {
    withRouter,
    Link
  } from "react-router-dom";
import { Routes } from '../component/Routes';

const { Header, Content, Footer, Sider } = Layout;

class SiderLayout extends React.Component {
  state = {
    collapsed: false,
  };

  onCollapse = collapsed => {
    this.setState({ collapsed });
  };

  render() {
    const { collapsed } = this.state
    const logoStyle = {
      color: "white",
      fontSize: 20,
      fontWeight: "bold",
      paddingLeft: 20
    }
    return (
      <Layout style={{ minHeight: '100vh' }}>
        <Sider collapsible collapsed={collapsed} onCollapse={this.onCollapse}>
          <Header className="site-layout-background" style={{ padding: 0 }}>
            <div style={logoStyle}>🐸 Rospo</div>
          </Header>
          <Menu theme="dark" defaultSelectedKeys={['1']} mode="inline">
            <Menu.Item key="1" icon={<FilterOutlined />}>
                <Link to="/">Tunnels</Link>
            </Menu.Item>
            <Menu.Item key="2" icon={<FunnelPlotOutlined />}>
              <Link to="/pipes">Pipes</Link>
            </Menu.Item>
          </Menu>
        </Sider>
        <Layout className="site-layout">
          <Header className="site-layout-background" style={{ padding: 0 }} />
          <Content style={{ margin: '0 16px' }}>
            {/* <Breadcrumb style={{ margin: '16px 0' }}>
              <Breadcrumb.Item>User</Breadcrumb.Item>
            </Breadcrumb> */}
            <div className="site-layout-background" style={{ padding: 24, minHeight: 360 }}>
              <Routes />
            </div>
          </Content>
          <Footer style={{ textAlign: 'center' }}>Rospo</Footer>
        </Layout>
      </Layout>
    );
  }
}

export const SiderLayoutWithRouter = withRouter(SiderLayout)
