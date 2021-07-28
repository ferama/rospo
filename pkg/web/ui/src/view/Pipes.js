import React from 'react';
import { http } from '../lib/Axios'
import { Table, PageHeader } from 'antd';
import { CreateForm } from '../components/CreateForm';
import { Row, Col, Button, Divider } from 'antd';
import {
    DeleteOutlined,
  } from '@ant-design/icons';

export class Pipes extends React.Component {
    constructor(props) {
        super(props)
        this.state = {
            pipes: [],
            showForm: false
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
        if (data.data === null) {
            this.setState({
                pipes: []
            })
            return
        }

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

    onNewClick = () => {
        const showForm = !this.state.showForm
        this.setState({
            showForm: showForm
        })
    }
    
    onFormFinish = async (values) => {
        await http.post('pipes/', values)
        this.setState({
            showForm: false
        })
    }

    onDelete = async (id) => {
        await http.delete(`/pipes/${id}`)
        await this.getAll()
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
            {
                title: 'Active Clients',
                dataIndex: 'ClientsCount',
                key: '7',
            },
            {
                title: 'Action',
                key: '8',
                render: (_, record) =>  (
                    <React.Fragment>
                        <Button onClick={ (e) => this.onDelete(record.Id)} >
                            <DeleteOutlined /> 
                        </Button>
                    </React.Fragment>
                ),
            }
        ]
        return (
            <React.Fragment>
                <Row>
                    <Col flex="auto">
                        <PageHeader title="Pipes" />
                    </Col>    
                    <Col flex="100px">
                        <Button type="primary" onClick={this.onNewClick}>New Pipe</Button>
                    </Col>    
                </Row>
                {this.state.showForm?(
                    <React.Fragment>
                        <CreateForm showForward={false} onFinish={this.onFormFinish}/>
                        <Divider />
                    </React.Fragment>
                ): ""}
                <Table columns={columns} dataSource={this.state.pipes} />
            </React.Fragment>
        )
    }
} 