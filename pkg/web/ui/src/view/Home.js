import React, { Fragment } from 'react';
import { http } from '../lib/Axios'
import { PageHeader } from 'antd';
import { Card } from 'antd';
import { List } from 'antd';
import { Statistic, Row, Col } from 'antd';


export class Home extends React.Component {
    constructor(props) {
        super(props)
        this.state = {
            "info": {},
            "stats": {}
        }
        this.intervalHandler = null
    }
    async componentDidMount() {
        let data
        try {
            data = await http.get("info")
        } catch {
            return
        }
        if (data.data === null) return
        this.setState({
            "info": data.data
        })

        await this.updateStats()
        this.intervalHandler = setInterval(this.updateStats, 5000)
    }

    componentWillUnmount() {
        clearInterval(this.intervalHandler)
    }

    updateStats = async () => {
        let data
        try {
            data = await http.get("stats")
        } catch {
            return
        }
        if (data.data === null) return
        this.setState({
            "stats": data.data
        })
    }

    render () {
        const haveJH = ( (this.state.info.JumpHosts !== undefined) && (this.state.info.JumpHosts.length !== 0) )
        return (
            <Fragment>
                <PageHeader
                    title="Home"
                />
                <Row gutter={16}>
                    <Col span={8}>
                        <Card title="Ssh Client">
                            <p>
                                <b>Server:</b> {this.state.info.SshClientURI}<br/>
                                <b>Status:</b> {this.state.info.SshClientConnectionStatus}
                            </p>
                            {haveJH ? (
                                <List
                                    header={<div>Jump Hosts</div>}
                                    bordered
                                    dataSource={this.state.info.JumpHosts}
                                    renderItem={item => (
                                        <List.Item>{item}</List.Item>
                                    )}
                                    />
                            ): ""}
                        </Card>
                    </Col>
                    <Col span={8}>
                        <Card title="Tunnels">
                            <Statistic title="Count" value={this.state.stats.CountTunnels} />
                            <Statistic title="Connected Clients" value={this.state.stats.CountTunnelsClients} />
                        </Card>
                    </Col>
                    <Col span={8}>
                        <Card title="Pipes">
                            <Statistic title="Count" value={this.state.stats.CountPipes} />
                            <Statistic title="Connected Clients" value={this.state.stats.CountPipesClients} />
                        </Card>
                    </Col>
                </Row>
               
            </Fragment>
        )
    }
} 