import React from 'react';
import PropTypes from 'prop-types';

import { Layout, Menu } from 'antd';
import {
  FunnelPlotOutlined,
  FilterOutlined,
  HomeOutlined
} from '@ant-design/icons';

import {
    withRouter,
    Link
  } from "react-router-dom";
import { Routes } from '../Routes';

const { Header, Content, Sider } = Layout;

class SiderLayout extends React.Component {
  static propTypes = {
    location: PropTypes.object.isRequired
  }
  state = {
    collapsed: false,
  };

  onCollapse = collapsed => {
    this.setState({ collapsed })
  };

  render() {
    const { collapsed } = this.state
    const { location } = this.props

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
            <Menu.Item key="/pipes" icon={<FunnelPlotOutlined />}>
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
          {/* <Footer style={{ textAlign: 'center' }}>Rospo</Footer> */}
        </Layout>
      </Layout>
    );
  }
}

export const SiderLayoutWithRouter = withRouter(SiderLayout)
