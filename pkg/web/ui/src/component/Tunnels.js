import React from 'react'
import { http } from '../lib/Axios'
import { Table, PageHeader } from 'antd';

export class Tunnels extends React.Component {
    constructor(props) {
        super(props)
        this.state = {
            tuns: []
        }
        this.intervalHandler = null
    }

    async componentDidMount() {
        await this.getAll()
        this.intervalHandler = setInterval(this.getAll, 3000)
    }
    componentWillUnmount() {
        clearInterval(this.intervalHandler)
    }

    getAll = async () => {
        const data = await http.get("tuns/")
        if (data.data === null) return
        
        const tuns = []

        for (let t of data.data) {
            t["key"] = parseInt(t.Id)
            tuns.push(t)
        }
        // always sort by key
        tuns.sort( (a, b) => {
            if (a.key < b.key) return -1
            if (a.key > b.key) return 1
            return 0
        })

        this.setState({
            tuns: tuns
        })
    }
    
    render () {
        const columns = [
            {
              title: 'Id',
              dataIndex: 'Id',
              key: '1',
            },
            {
                title: 'Listener IP',
                dataIndex: 'Listener',
                key: '2',
                render: item => item.IP
            },
            {
                title: 'Listener Port',
                dataIndex: 'Listener',
                key: '3',
                render: item => item.Port
            },
            {
                title: 'Is Local Listener',
                dataIndex: 'IsListenerLocal',
                key: '4',
                render: item => item ? "true": "false"
            },
            {
                title: 'Endpoint Host',
                dataIndex: 'Endpoint',
                key: '5',
                render: item => item.Host
            },
            {
                title: 'Endpoint Port',
                dataIndex: 'Endpoint',
                key: '6',
                render: item => item.Port
            },
        ]
        return (
            <React.Fragment>
                <PageHeader
                    title="Tunnels"
                />
                <Table columns={columns} dataSource={this.state.tuns} />
            </React.Fragment>
        )
    }
} 