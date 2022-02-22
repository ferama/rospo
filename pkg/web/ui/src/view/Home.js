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
        await this.updateState()
        this.intervalHandler = setInterval(this.updateState, 5000)
    }

    formatBytes(bytes, decimals = 2) {
        if (bytes === 0) return '0 Bytes'
    
        const k = 1024
        const dm = decimals < 0 ? 0 : decimals
        const sizes = ['Bytes', 'KB', 'MB', 'GB', 'TB', 'PB', 'EB', 'ZB', 'YB']
    
        const i = Math.floor(Math.log(bytes) / Math.log(k))
    
        return parseFloat((bytes / Math.pow(k, i)).toFixed(dm)) + ' ' + sizes[i]
    }

    componentWillUnmount() {
        clearInterval(this.intervalHandler)
    }

    updateState = async () => {
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
                <Row>
                    <Col span={12}>
                        <Card title="SSH Client">
                            <Statistic title="Server" value={this.state.info.SshClientURI} />
                            <Statistic title="Status" value={this.state.info.SshClientConnectionStatus} />
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
                    <Col span={12}>
                        <Card title="Tunnels">
                            <Row gutter={16}>
                                <Col span={12}>
                                    <Statistic title="Active Tunnels" value={this.state.stats.CountTunnels} />
                                </Col>
                                <Col span={12}>
                                    <Statistic title="Connected Clients" value={this.state.stats.CountTunnelsClients} />
                                </Col>
                            </Row>
                            <Statistic title="Total Throughput" value={this.state.stats.TotalTunnelThroughputString} />
                        </Card>
                    </Col>
                </Row>
                <Row>
                    <Col span={12}>
                        <Card title="Global Stats">
                            <Statistic title="GoRoutines" value={this.state.stats.NumGoroutine} />
                            <Statistic title="Allocated Memory" value={this.formatBytes(this.state.stats.MemTotal)} />
                        </Card>
                    </Col>
                    <Col span={12}>
                        <Card title="Pipes">
                            <Row gutter={16}>
                                <Col span={12}>
                                    <Statistic title="Active Pipes" value={this.state.stats.CountPipes} />
                                </Col>
                                <Col span={12}>
                                    <Statistic title="Connected Clients" value={this.state.stats.CountPipesClients} />
                                </Col>
                            </Row>
                            <Statistic title="Total Throughput" value={this.state.stats.TotalPipeThroughputString} />
                        </Card>
                    </Col>
                </Row>
               
            </Fragment>
        )
    }
} 