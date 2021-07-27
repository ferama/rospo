import React, { Fragment } from 'react';
import { http } from '../lib/Axios'
import { PageHeader } from 'antd';
import { Card } from 'antd';
import { List } from 'antd';

export class Home extends React.Component {
    constructor(props) {
        super(props)
        this.state = {
            "info": {}
        }
       
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
    }

    render () {
        const haveJH = ( (this.state.info.JumpHosts !== undefined) && (this.state.info.JumpHosts.length !== 0) )
        return (
            <Fragment>
                <PageHeader
                    title="Home"
                />
                <Card title="Ssh Client">
                    <p>
                        <b>Connected to:</b> {this.state.info.SshClientURI}
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
                    ): (
                        <Fragment></Fragment>
                    )}
                </Card>
            </Fragment>
        )
    }
} 