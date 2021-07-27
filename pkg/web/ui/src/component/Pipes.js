import React from 'react';
import { http } from '../lib/Axios'
import { Table, PageHeader } from 'antd';

export class Pipes extends React.Component {
    constructor(props) {
        super(props)
        this.state = {
            pipes: []
        }
        this.intervalHandler = null
    }

    async componentDidMount() {
        await this.getAll()
        this.intervalHandler = setInterval(this.getAll, 5000)
    }

    componentWillUnmount() {
        clearInterval(this.intervalHandler)
    }

    getAll = async () => {
        let data
        try {
            data = await http.get("pipes/")
        } catch {
            return
        }
        if (data.data === null) return

        const pipes = []

        for (let t of data.data) {
            t["key"] = parseInt(t.Id)
            pipes.push(t)
        }
        // always sort by key
        pipes.sort( (a, b) => {
            if (a.key < b.key) return -1
            if (a.key > b.key) return 1
            return 0
        })

        this.setState({
            pipes: pipes
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
                title: 'Endpoint Host',
                dataIndex: 'Endpoint',
                key: '4',
                render: item => item.Host
            },
            {
                title: 'Endpoint Port',
                dataIndex: 'Endpoint',
                key: '5',
                render: item => item.Port
            },
        ]
        return (
            <React.Fragment>
                <PageHeader
                    title="Pipes"
                />
                <Table columns={columns} dataSource={this.state.pipes} />
            </React.Fragment>
        )
    }
} 